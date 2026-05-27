package main

import (
	"fmt"
	"io"

	"github.com/drogers0/llm-usage/internal/httpx"
	"github.com/drogers0/llm-usage/internal/providers"
	"github.com/drogers0/llm-usage/internal/providers/claude"
	"github.com/drogers0/llm-usage/internal/providers/codex"
	"github.com/drogers0/llm-usage/internal/providers/copilot"
)

// realProviders returns the live providers. When debug is non-nil, all three
// providers' Doers share a single ConcurrencySafeWriter so concurrent debug
// lines from different goroutines do not interleave mid-line on stderr.
func realProviders(debug io.Writer) []providers.Provider {
	var safeDebug io.Writer
	if debug != nil {
		safeDebug = &httpx.ConcurrencySafeWriter{W: debug}
	}
	var copilotOpts []copilot.Option
	if safeDebug != nil {
		copilotOpts = append(copilotOpts, copilot.WithWarn(func(s string) {
			fmt.Fprintln(safeDebug, "[debug] copilot: "+s)
		}))
	}
	return []providers.Provider{
		claude.New(safeDebug),
		codex.New(safeDebug),
		copilot.New(safeDebug, copilotOpts...),
	}
}

// fakeProviders returns deterministic in-process providers used by the
// undocumented --fake flag for end-to-end CLI tests.
func fakeProviders() []providers.Provider {
	return []providers.Provider{
		newFakeProvider("claude"),
		newFakeProvider("codex"),
		newFakeProvider("copilot"),
	}
}
