//go:build live

package codex

import (
	"context"
	"errors"
	"testing"

	"github.com/drogers0/llm-usage/internal/providers"
)

func TestLive_RealAuthAndEndpoint(t *testing.T) {
	c := New()
	out, err := c.Fetch(context.Background())
	if err != nil {
		if errors.Is(err, providers.ErrAuthMissing) {
			t.Skipf("no Codex token at ~/.codex/auth.json; skipping live test: %v", err)
		}
		t.Fatalf("live Fetch failed: %v", err)
	}
	if len(out.Limits) == 0 {
		t.Fatal("live Fetch returned no limits — possible API breakage or empty account")
	}
	t.Logf("live limits: %+v", out.Limits)
}
