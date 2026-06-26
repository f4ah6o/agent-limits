package main

import (
	"io"
	"testing"

	"github.com/f4ah6o/aistat/v2/internal/httpx"
)

func TestRealProvidersAreReadOnlyForkScope(t *testing.T) {
	serialStderr := httpx.NewConcurrencySafeWriter(io.Discard)

	chosen := realProviders(serialStderr, false, false)
	if len(chosen) != 2 {
		t.Fatalf("expected 2 providers, got %d", len(chosen))
	}

	if chosen[0].ID() != "claude" {
		t.Fatalf("provider[0] = %q, want claude", chosen[0].ID())
	}
	if chosen[1].ID() != "codex" {
		t.Fatalf("provider[1] = %q, want codex", chosen[1].ID())
	}
}
