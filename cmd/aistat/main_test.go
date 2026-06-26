package main

import (
	"bytes"
	"strings"
	"testing"
)

type runResult struct {
	stdout string
	stderr string
	code   int
}

func runCLI(args ...string) runResult {
	var stdout, stderr bytes.Buffer
	code := run(args, &stdout, &stderr)
	return runResult{stdout.String(), stderr.String(), code}
}

func wantExit(t *testing.T, r runResult, want int) {
	t.Helper()
	if r.code != want {
		t.Fatalf("expected exit %d, got %d\nstdout: %s\nstderr: %s", want, r.code, r.stdout, r.stderr)
	}
}

func wantOut(t *testing.T, r runResult, sub string) {
	t.Helper()
	if !strings.Contains(r.stdout, sub) {
		t.Errorf("stdout missing %q\nstdout: %s", sub, r.stdout)
	}
}

func wantErrOut(t *testing.T, r runResult, sub string) {
	t.Helper()
	if !strings.Contains(r.stderr, sub) {
		t.Errorf("stderr missing %q\nstderr: %s", sub, r.stderr)
	}
}

func TestCLIHelpVersion(t *testing.T) {
	r := runCLI("--help")
	wantExit(t, r, 0)
	wantOut(t, r, "aistat")
	wantOut(t, r, "claude, codex")
	wantOut(t, r, "Read-only")

	r = runCLI("--version")
	wantExit(t, r, 0)
	if got := strings.TrimSpace(r.stdout); got == "" {
		t.Fatalf("expected non-empty version, got empty")
	}
}

func TestCLIGlobalFlags(t *testing.T) {
	r := runCLI("--debug=true", "usage")
	wantExit(t, r, 2)
	wantErrOut(t, r, "--flag=value form not supported for global flags")

	for _, args := range [][]string{{"--human", "usage"}, {"usage", "--human"}} {
		r := runCLI(args...)
		if r.code == 2 {
			t.Fatalf("args %v should not produce usage error: %s", args, r.stderr)
		}
	}
}

func TestCLIBadInput(t *testing.T) {
	r := runCLI("unknown-subcmd")
	wantExit(t, r, 2)
	wantErrOut(t, r, `unknown subcommand "unknown-subcmd"`)
}

func TestCLIUsageProviderValidation(t *testing.T) {
	r := runCLI("usage", "unknown-provider")
	wantExit(t, r, 2)
	wantErrOut(t, r, "provider must be one of claude, codex")

	r = runCLI("usage", "copilot")
	wantExit(t, r, 2)
	wantErrOut(t, r, "provider must be one of claude, codex")
}
