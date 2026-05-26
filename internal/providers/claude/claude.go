package claude

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"time"

	"github.com/drogers0/llm-usage/internal/cred"
	"github.com/drogers0/llm-usage/internal/providers"
)

const (
	endpoint   = "https://api.anthropic.com/api/oauth/usage"
	betaHeader = "oauth-2025-04-20"
	userAgent  = "usage-check/v2 (+https://github.com/drogers0/llm-usage)"
	timeout    = 10 * time.Second
)

// usageWindows is the closed set of API keys we surface, mapped to normalized
// names. Adding a row here is half of the forward-compat step when Anthropic
// adds a new public window — the other half is internal/render/text.go's
// textLabels["claude"] (see T1's D15).
var usageWindows = []struct{ apiKey, normalized string }{
	{"five_hour", "five_hour"},
	{"seven_day", "seven_day"},
	{"seven_day_sonnet", "seven_day_sonnet"},
	// Any window not listed above (seven_day_opus, seven_day_oauth_apps,
	// seven_day_cowork, seven_day_omelette, tangelo, iguana_necktie,
	// omelette_promotional, …) is intentionally filtered out. The set is
	// closed; Anthropic adds public windows infrequently.
}

type Client struct {
	http      *http.Client
	endpoint  string
	readToken func(context.Context) (string, error)
}

func New() *Client {
	return &Client{
		http:      &http.Client{Timeout: timeout},
		endpoint:  endpoint,
		readToken: cred.ReadClaudeToken,
	}
}

func (c *Client) ID() string { return "claude" }

// URL returns the upstream endpoint. T5's orchestrator may use this for
// --debug logging; the field is not on the Provider interface.
func (c *Client) URL() string { return c.endpoint }

type window struct {
	Utilization float64 `json:"utilization"`
	ResetsAt    *string `json:"resets_at"`
}

func (c *Client) Fetch(ctx context.Context) (providers.ProviderOutput, error) {
	token, err := c.readToken(ctx)
	if err != nil {
		if errors.Is(err, cred.ErrClaudeTokenNotFound) {
			return providers.ProviderOutput{}, fmt.Errorf("%w: %s", providers.ErrAuthMissing, err.Error())
		}
		return providers.ProviderOutput{}, err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.endpoint, nil)
	if err != nil {
		return providers.ProviderOutput{}, err
	}
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("anthropic-beta", betaHeader)
	req.Header.Set("User-Agent", userAgent)

	resp, err := c.http.Do(req)
	if err != nil {
		// Cancellation is the caller's deliberate action, not a retryable failure.
		if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
			return providers.ProviderOutput{}, err
		}
		return providers.ProviderOutput{}, fmt.Errorf("%w: %s", providers.ErrTransient, err.Error())
	}
	defer resp.Body.Close()

	body, _ := io.ReadAll(resp.Body)
	switch {
	case resp.StatusCode == 401 || resp.StatusCode == 403:
		return providers.ProviderOutput{}, fmt.Errorf("%w: HTTP %d: %s", providers.ErrAuthDenied, resp.StatusCode, snip(body))
	case resp.StatusCode == 408 || resp.StatusCode == 429 || resp.StatusCode >= 500:
		return providers.ProviderOutput{}, fmt.Errorf("%w: HTTP %d: %s", providers.ErrTransient, resp.StatusCode, snip(body))
	case resp.StatusCode != 200:
		return providers.ProviderOutput{}, fmt.Errorf("Claude usage endpoint returned HTTP %d: %s", resp.StatusCode, snip(body))
	}

	var raw map[string]*window
	if err := json.Unmarshal(body, &raw); err != nil {
		return providers.ProviderOutput{}, fmt.Errorf("Claude usage endpoint returned non-JSON: %w", err)
	}

	// Note: this `now` and the orchestrator's checked_at are computed from
	// separate time.Now() calls, so reset_after_seconds + checked_at may
	// differ from resets_at by up to one second. Accepted per T2 D3.
	now := time.Now().UTC().Truncate(time.Second)
	limits := map[string]providers.Limit{}
	for _, w := range usageWindows {
		win := raw[w.apiKey]
		if win == nil || win.ResetsAt == nil {
			continue
		}
		resets, err := time.Parse(time.RFC3339Nano, *win.ResetsAt)
		if err != nil {
			return providers.ProviderOutput{}, fmt.Errorf("Claude window %s has unparseable resets_at %q: %w", w.apiKey, *win.ResetsAt, err)
		}
		resets = resets.UTC().Truncate(time.Second)
		secs := int(resets.Sub(now).Seconds())
		if secs < 0 {
			secs = 0
		}
		limits[w.normalized] = providers.Limit{
			UsedPercent:       win.Utilization,
			RemainingPercent:  100 - win.Utilization,
			ResetsAt:          resets,
			ResetAfterSeconds: secs,
		}
	}
	if len(limits) == 0 {
		// Nil out so ProviderResult.Limits is omitempty-suppressed (the
		// map field's omitempty hides nil but not an empty-non-nil map).
		limits = nil
	}
	return providers.ProviderOutput{Limits: limits}, nil
}

func snip(b []byte) string {
	s := string(b)
	if len(s) > 200 {
		return s[:200] + "..."
	}
	return s
}
