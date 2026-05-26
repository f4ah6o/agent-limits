package copilot

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"net/http"
	"strings"
	"time"

	"github.com/drogers0/llm-usage/internal/cred"
	"github.com/drogers0/llm-usage/internal/providers"
)

const (
	userEndpoint      = "https://api.github.com/user"
	usageEndpointTmpl = "https://api.github.com/users/%s/settings/billing/premium_request/usage?year=%d&month=%d"
	userAgent         = "usage-check/v2 (+https://github.com/drogers0/llm-usage)"
	acceptHeader      = "application/vnd.github+json"
	timeout           = 10 * time.Second
	defaultQuota      = 300
)

// planQuota maps GitHub Copilot plan slugs (from /user.plan.name) to their
// monthly premium-request quotas.
//
// Source: https://docs.github.com/en/copilot/get-started/plans (verified
// 2026-05-26). GitHub returns "business" for the org-billed plan; the "team"
// alias is included defensively — harmless if it ever appears.
var planQuota = map[string]int{
	"free":       50,
	"pro":        300,
	"pro_plus":   1500,
	"business":   300,
	"team":       300,
	"enterprise": 1000,
}

type Client struct {
	http      *http.Client
	readToken func(context.Context) (string, error)
	userURL   string
	usageURL  func(login string, year int, month int) string
	warn      func(string)
}

// Option mutates a Client at construction time.
type Option func(*Client)

// WithWarn installs a callback the provider uses to surface non-fatal warnings
// (e.g. unknown plan name, SKU filter mismatch). T5's debug decorator passes a
// stderr writer here when --debug is set; main passes nothing otherwise.
//
// The callback may be invoked from the Fetch goroutine; if the caller's
// closure touches shared state, the caller is responsible for synchronization.
func WithWarn(fn func(string)) Option { return func(c *Client) { c.warn = fn } }

func New(opts ...Option) *Client {
	c := &Client{
		http:      &http.Client{}, // ctx-scoped deadline replaces a per-client Timeout.
		readToken: cred.ReadGitHubToken,
		userURL:   userEndpoint,
		usageURL: func(login string, year int, month int) string {
			return fmt.Sprintf(usageEndpointTmpl, login, year, month)
		},
	}
	for _, o := range opts {
		o(c)
	}
	return c
}

func (c *Client) ID() string  { return "copilot" }
func (c *Client) URL() string { return c.userURL }

type userResp struct {
	Login string `json:"login"`
	Plan  struct {
		Name string `json:"name"`
	} `json:"plan"`
}

type usageItem struct {
	Product       string  `json:"product"`
	Sku           string  `json:"sku"`
	GrossQuantity float64 `json:"grossQuantity"`
}

type usageResp struct {
	UsageItems []usageItem `json:"usageItems"`
}

func (c *Client) Fetch(ctx context.Context) (providers.ProviderOutput, error) {
	// Single shared budget for both HTTP calls — matches T2/T3's implicit 10s/provider.
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	token, err := c.readToken(ctx)
	if err != nil {
		if errors.Is(err, cred.ErrGitHubTokenNotFound) {
			return providers.ProviderOutput{}, fmt.Errorf("%w: %s", providers.ErrAuthMissing, err.Error())
		}
		return providers.ProviderOutput{}, err
	}

	var user userResp
	if err := c.get(ctx, token, c.userURL, &user); err != nil {
		return providers.ProviderOutput{}, err
	}
	if user.Login == "" {
		return providers.ProviderOutput{}, errors.New("GitHub /user returned empty login")
	}

	quota := defaultQuota
	if q, ok := planQuota[user.Plan.Name]; ok {
		quota = q
	} else if c.warn != nil {
		c.warn(fmt.Sprintf("Copilot: unknown plan name %q; falling back to %d/month quota", user.Plan.Name, defaultQuota))
	}

	// See claude.go: this `now` and main's checked_at are computed from
	// separate time.Now() calls, so reset_after_seconds + checked_at may
	// differ from resets_at by up to one second. Accepted per T2 D3.
	now := time.Now().UTC()

	var usage usageResp
	if err := c.get(ctx, token, c.usageURL(user.Login, now.Year(), int(now.Month())), &usage); err != nil {
		return providers.ProviderOutput{}, err
	}

	var gross float64
	for _, it := range usage.UsageItems {
		if it.Product == "Copilot" && it.Sku == "Copilot Premium Request" {
			gross += it.GrossQuantity
		}
	}
	// Silent-degradation tripwire: if items existed but none matched the
	// filter, GitHub probably changed the Product/Sku spelling. We still
	// emit 0%, but a warning lets the user know the result is suspect.
	if len(usage.UsageItems) > 0 && gross == 0 && c.warn != nil {
		c.warn(fmt.Sprintf("Copilot: %d usageItems received but none matched product=\"Copilot\" sku=\"Copilot Premium Request\"; result may be incorrect", len(usage.UsageItems)))
	}
	// Clamp to 100: the Limit contract uses a [0,100] convention. Overage
	// detail lives in the API's discountQuantity/netQuantity fields but is
	// out of scope for this contract.
	//
	// Round to two decimal places: float64 division produces ugly artifacts
	// like 67.33999999999999. Sub-percentage precision adds no user value.
	used := math.Round(math.Min(100, (gross/float64(quota))*100)*100) / 100

	year, month, _ := now.Date()
	reset := time.Date(year, month+1, 1, 0, 0, 0, 0, time.UTC) // month=13 → Jan next year (Go normalizes).
	secs := int(reset.Sub(now).Seconds())
	if secs < 0 {
		secs = 0
	}

	return providers.ProviderOutput{Limits: map[string]providers.Limit{
		"month": {
			UsedPercent:       used,
			RemainingPercent:  math.Round((100-used)*100) / 100,
			ResetsAt:          reset,
			ResetAfterSeconds: secs,
		},
	}}, nil
}

func (c *Client) get(ctx context.Context, token, url string, dst any) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return err
	}
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("Accept", acceptHeader)
	req.Header.Set("User-Agent", userAgent)
	resp, err := c.http.Do(req)
	if err != nil {
		if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
			return err
		}
		return fmt.Errorf("%w: %s", providers.ErrTransient, err.Error())
	}
	defer resp.Body.Close()
	body, _ := io.ReadAll(resp.Body)
	switch {
	case resp.StatusCode == 401 || resp.StatusCode == 403:
		return fmt.Errorf("%w: HTTP %d: %s", providers.ErrAuthDenied, resp.StatusCode, snip(body))
	case resp.StatusCode == 404 && strings.Contains(string(body), "Not Found"):
		// Missing `user` scope is the dominant cause; preceding /user call
		// already proved the token and login both work, so 404 here is not
		// "user does not exist" in practice.
		return fmt.Errorf("%w: %s", providers.ErrAuthMissing, cred.GitHubTokenMissingMessage)
	case resp.StatusCode == 408 || resp.StatusCode == 429 || resp.StatusCode >= 500:
		return fmt.Errorf("%w: HTTP %d: %s", providers.ErrTransient, resp.StatusCode, snip(body))
	case resp.StatusCode != 200:
		return fmt.Errorf("GitHub returned HTTP %d for %s: %s", resp.StatusCode, url, snip(body))
	}
	if err := json.Unmarshal(body, dst); err != nil {
		return fmt.Errorf("GitHub returned non-JSON from %s: %w", url, err)
	}
	return nil
}

func snip(b []byte) string {
	s := string(b)
	if len(s) > 200 {
		return s[:200] + "..."
	}
	return s
}
