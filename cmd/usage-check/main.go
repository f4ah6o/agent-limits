package main

import (
	"context"
	"flag"
	"fmt"
	"io"
	"os"
	"os/signal"
	"strings"

	"github.com/drogers0/llm-usage/internal/orchestrate"
	"github.com/drogers0/llm-usage/internal/providers"
	"github.com/drogers0/llm-usage/internal/render"
)

var knownServices = map[string]bool{"claude": true, "codex": true, "copilot": true}

const helpText = `usage-check — report Claude, Codex, and Copilot usage

Usage:
  usage-check [provider] [flags]

Providers (optional):
  claude    Only query Claude
  codex     Only query Codex
  copilot   Only query Copilot
  (none)    Query all providers

Flags:
  -h, --human   Render human-readable text instead of JSON (default JSON)
      --debug   Write per-provider URL, status, and timing to stderr
      --help    Print this help and exit
`

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

func run(args []string, stdout, stderr io.Writer) int {
	service, rest, err := extractService(args)
	if err != nil {
		fmt.Fprintln(stderr, err.Error())
		return 2
	}

	fs := flag.NewFlagSet("usage-check", flag.ContinueOnError)
	fs.SetOutput(io.Discard)
	fs.Usage = func() {} // silence default Usage; we print our own help on --help.

	human := fs.Bool("human", false, "")
	fs.BoolVar(human, "h", false, "")
	debug := fs.Bool("debug", false, "")
	help := fs.Bool("help", false, "")
	fake := fs.Bool("fake", false, "") // undocumented

	if err := fs.Parse(rest); err != nil {
		fmt.Fprintln(stderr, err.Error())
		return 2
	}

	if *help {
		fmt.Fprint(stdout, helpText)
		return 0
	}

	if fs.NArg() > 0 {
		fmt.Fprintf(stderr, "unexpected argument: %s (providers must be one of claude, codex, copilot)\n", fs.Arg(0))
		return 2
	}

	requested := selectedProviders(service)

	var debugWriter io.Writer
	if *debug {
		debugWriter = stderr
	}

	var chosen []providers.Provider
	if *fake {
		chosen = fakeProviders()
	} else {
		chosen = realProviders(debugWriter)
	}

	available := map[string]providers.Provider{}
	for _, p := range chosen {
		available[p.ID()] = p
	}
	var missing []string
	for _, id := range requested {
		if _, ok := available[id]; !ok {
			missing = append(missing, id)
		}
	}
	if len(missing) > 0 {
		fmt.Fprintf(stderr, "provider not available: %s (run with --fake to exercise the CLI surface)\n", strings.Join(missing, ", "))
		return 2
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()

	report, status := orchestrate.Run(ctx, requested, chosen, orchestrate.Options{Debug: debugWriter})

	var renderErr error
	if *human {
		renderErr = render.Text(stdout, report, requested)
	} else {
		renderErr = render.JSON(stdout, report)
	}
	if renderErr != nil {
		fmt.Fprintln(stderr, renderErr.Error())
		return 2
	}
	return int(status)
}

// extractService walks args once, pulls out the single positional
// provider name (claude/codex/copilot), and returns the remaining args.
// Two providers is an error; zero is "all".
func extractService(args []string) (service string, rest []string, err error) {
	rest = append([]string(nil), args...)
	for i := 0; i < len(rest); i++ {
		if !knownServices[rest[i]] {
			continue
		}
		if service != "" {
			return "", nil, fmt.Errorf("multiple providers specified: %s, %s", service, rest[i])
		}
		service = rest[i]
		rest = append(rest[:i:i], rest[i+1:]...)
		i--
	}
	return service, rest, nil
}

func selectedProviders(service string) []string {
	if service == "" {
		return []string{"claude", "codex", "copilot"}
	}
	return []string{service}
}
