package main

import (
	"fmt"
	"io"

	"github.com/drogers0/llm-usage/internal/providers"
	"github.com/drogers0/llm-usage/internal/providers/claude"
	"github.com/drogers0/llm-usage/internal/providers/codex"
	"github.com/drogers0/llm-usage/internal/providers/copilot"
)

// realProviders returns the live providers. When debug is non-nil, the
// Copilot provider routes its warn callback there.
func realProviders(debug io.Writer) []providers.Provider {
	var copilotOpts []copilot.Option
	if debug != nil {
		copilotOpts = append(copilotOpts, copilot.WithWarn(func(s string) {
			fmt.Fprintln(debug, "[debug] "+s)
		}))
	}
	return []providers.Provider{claude.New(), codex.New(), copilot.New(copilotOpts...)}
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
