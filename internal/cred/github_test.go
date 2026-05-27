package cred

import (
	"context"
	"errors"
	"os/exec"
	"testing"
)

// TestReadGitHubToken_ContextCancelledStillWrapsSentinel verifies the fallback
// branch in ReadGitHubToken: when cmd.Output() returns an error that is neither
// *exec.ExitError nor *exec.Error (e.g., context.Canceled from a pre-cancelled
// ctx), the error must still wrap ErrGitHubTokenNotFound so upstream callers
// classify it as ErrAuthMissing.
//
// Requires `gh` on PATH; skips otherwise.
//
// Behavior note (Go 1.22): exec.CommandContext with a pre-cancelled ctx
// returns context.Canceled from cmd.Start(), which is neither *exec.ExitError
// nor *exec.Error — exercising the fallback branch as intended. If a future
// Go version reclassifies this error, the test will fail on the wrong branch;
// treat it as a tripwire.
func TestReadGitHubToken_ContextCancelledStillWrapsSentinel(t *testing.T) {
	if _, err := exec.LookPath("gh"); err != nil {
		t.Skip("gh not on PATH; skipping")
	}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := ReadGitHubToken(ctx)
	if err == nil {
		t.Fatal("expected error from pre-cancelled ctx")
	}
	if !errors.Is(err, ErrGitHubTokenNotFound) {
		t.Errorf("error should wrap ErrGitHubTokenNotFound, got: %v", err)
	}
}
