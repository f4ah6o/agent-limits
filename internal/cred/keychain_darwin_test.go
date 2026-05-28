//go:build darwin

package cred

import (
	"bytes"
	"context"
	"os"
	"os/exec"
	"slices"
	"strings"
	"testing"
)

// TestWriteClaudeLiveBlob_SeamArgs verifies that WriteClaudeLiveBlob invokes
// the security(1) tool with the exact arguments required by D15:
//
//  1. add-generic-password -U -s "Claude Code-credentials" -a <user> -w <blob>
//  2. set-generic-password-partition-list -S "apple-tool:,apple:" -s "Claude Code-credentials" -a <user>
//
// The runSecurity seam is replaced for the duration of this test so no real
// keychain access occurs. This test always runs (no AISTAT_LIVE_KEYCHAIN guard).
func TestWriteClaudeLiveBlob_SeamArgs(t *testing.T) {
	blob := []byte(`{"claudeAiOauth":{"accessToken":"test-tok"}}`)
	u := claudeUser()

	type call struct{ args []string }
	var calls []call

	orig := runSecurity
	t.Cleanup(func() { runSecurity = orig })
	runSecurity = func(ctx context.Context, args ...string) ([]byte, error) {
		calls = append(calls, call{args: append([]string(nil), args...)})
		return nil, nil
	}

	if err := WriteClaudeLiveBlob(context.Background(), blob); err != nil {
		t.Fatalf("WriteClaudeLiveBlob: %v", err)
	}

	if len(calls) != 2 {
		t.Fatalf("expected 2 security calls, got %d", len(calls))
	}

	// Step 1: add-generic-password
	want1 := []string{
		"add-generic-password", "-U",
		"-s", "Claude Code-credentials",
		"-a", u,
		"-w", string(blob),
	}
	if !slices.Equal(calls[0].args, want1) {
		t.Errorf("call[0] args:\ngot:  %v\nwant: %v", calls[0].args, want1)
	}

	// Step 2: set-generic-password-partition-list
	want2 := []string{
		"set-generic-password-partition-list",
		"-S", "apple-tool:,apple:",
		"-s", "Claude Code-credentials",
		"-a", u,
	}
	if !slices.Equal(calls[1].args, want2) {
		t.Errorf("call[1] args:\ngot:  %v\nwant: %v", calls[1].args, want2)
	}
}

// TestWriteClaudeLiveBlob_SeamStep2NotCalledOnStep1Error verifies that if
// add-generic-password fails, set-generic-password-partition-list is not
// attempted and the error is propagated.
func TestWriteClaudeLiveBlob_SeamStep2NotCalledOnStep1Error(t *testing.T) {
	blob := []byte(`{"claudeAiOauth":{"accessToken":"tok"}}`)

	var calls int
	orig := runSecurity
	t.Cleanup(func() { runSecurity = orig })
	runSecurity = func(ctx context.Context, args ...string) ([]byte, error) {
		calls++
		// Return a non-ExitError so the error falls through to the %w wrap in
		// WriteClaudeLiveBlob. ctx.Err() returns nil (context is not cancelled),
		// so this exercises the plain-error branch, not the context branch.
		return nil, &exec.Error{Name: "security", Err: os.ErrPermission}
	}

	err := WriteClaudeLiveBlob(context.Background(), blob)
	if err == nil {
		t.Fatal("expected error when step 1 fails")
	}
	if calls != 1 {
		t.Errorf("expected exactly 1 security call (step 1 only), got %d", calls)
	}
}

// TestReadClaudeCredential_Seam verifies that ReadClaudeCredential uses the
// runSecurity seam and correctly parses the returned JSON. Always runs.
func TestReadClaudeCredential_Seam(t *testing.T) {
	payload := []byte(`{"claudeAiOauth":{"accessToken":"seam-tok","refreshToken":"rt","expiresAt":99}}`)

	orig := runSecurity
	t.Cleanup(func() { runSecurity = orig })
	runSecurity = func(ctx context.Context, args ...string) ([]byte, error) {
		return payload, nil
	}

	c, err := ReadClaudeCredential(context.Background())
	if err != nil {
		t.Fatalf("ReadClaudeCredential: %v", err)
	}
	if c.AccessToken != "seam-tok" {
		t.Errorf("AccessToken: got %q, want %q", c.AccessToken, "seam-tok")
	}
	if c.RefreshToken != "rt" {
		t.Errorf("RefreshToken: got %q, want %q", c.RefreshToken, "rt")
	}
	if c.ExpiresAt != 99 {
		t.Errorf("ExpiresAt: got %d, want %d", c.ExpiresAt, 99)
	}
	if !bytes.Equal(c.Raw, payload) {
		t.Errorf("Raw mismatch\ngot:  %q\nwant: %q", c.Raw, payload)
	}
}

// TestReadClaudeCredential_Seam_TrailingNewline verifies that security(1)'s
// trailing newline is stripped (exactly one) before bytes are stored in
// Credential.Raw, and that a payload already ending in '\n' does not have both
// newlines stripped.
func TestReadClaudeCredential_Seam_TrailingNewline(t *testing.T) {
	t.Run("single newline stripped", func(t *testing.T) {
		payload := []byte(`{"claudeAiOauth":{"accessToken":"seam-tok"}}`)
		withNewline := append(append([]byte(nil), payload...), '\n')

		orig := runSecurity
		t.Cleanup(func() { runSecurity = orig })
		runSecurity = func(ctx context.Context, args ...string) ([]byte, error) {
			return withNewline, nil
		}

		c, err := ReadClaudeCredential(context.Background())
		if err != nil {
			t.Fatalf("ReadClaudeCredential: %v", err)
		}
		if !bytes.Equal(c.Raw, payload) {
			t.Errorf("Raw should not contain trailing newline\ngot:  %q\nwant: %q", c.Raw, payload)
		}
	})

	t.Run("payload-internal newline preserved", func(t *testing.T) {
		// A JSON payload that ends with '\n' before security appends another.
		// TrimSuffix must remove exactly one '\n', not both.
		payloadWithNL := []byte("{\"claudeAiOauth\":{\"accessToken\":\"tok\"}}\n")
		fromSecurity := append(append([]byte(nil), payloadWithNL...), '\n')

		orig := runSecurity
		t.Cleanup(func() { runSecurity = orig })
		runSecurity = func(ctx context.Context, args ...string) ([]byte, error) {
			return fromSecurity, nil
		}

		c, err := ReadClaudeCredential(context.Background())
		if err != nil {
			t.Fatalf("ReadClaudeCredential: %v", err)
		}
		if !bytes.Equal(c.Raw, payloadWithNL) {
			t.Errorf("Raw should preserve payload-internal newline\ngot:  %q\nwant: %q", c.Raw, payloadWithNL)
		}
	})
}

// --- Live keychain tests (require AISTAT_LIVE_KEYCHAIN=1) ---

func TestWriteClaudeLiveBlob_LiveKeychain(t *testing.T) {
	if os.Getenv("AISTAT_LIVE_KEYCHAIN") != "1" {
		t.Skip("skipping live keychain test (set AISTAT_LIVE_KEYCHAIN=1 to enable)")
	}

	ctx := context.Background()
	// Backup the current credential so we can restore it.
	orig, backupErr := ReadClaudeCredential(ctx)
	t.Cleanup(func() {
		if backupErr == nil {
			if err := WriteClaudeLiveBlob(context.Background(), orig.Raw); err != nil {
				t.Logf("warning: failed to restore keychain credential: %v", err)
			}
		}
	})

	sentinel := []byte(`{"claudeAiOauth":{"accessToken":"aistat-live-test-sentinel","refreshToken":"","expiresAt":0}}`)
	if err := WriteClaudeLiveBlob(ctx, sentinel); err != nil {
		t.Fatalf("WriteClaudeLiveBlob: %v", err)
	}

	got, err := ReadClaudeCredential(ctx)
	if err != nil {
		t.Fatalf("ReadClaudeCredential after write: %v", err)
	}
	if got.AccessToken != "aistat-live-test-sentinel" {
		t.Errorf("AccessToken: got %q, want %q", got.AccessToken, "aistat-live-test-sentinel")
	}
}

func TestWriteClaudeLiveBlob_LiveKeychain_PartitionList(t *testing.T) {
	if os.Getenv("AISTAT_LIVE_KEYCHAIN") != "1" {
		t.Skip("skipping live keychain test (set AISTAT_LIVE_KEYCHAIN=1 to enable)")
	}

	ctx := context.Background()
	orig, backupErr := ReadClaudeCredential(ctx)
	t.Cleanup(func() {
		if backupErr == nil {
			if err := WriteClaudeLiveBlob(context.Background(), orig.Raw); err != nil {
				t.Logf("warning: failed to restore keychain credential: %v", err)
			}
		}
	})

	sentinel := []byte(`{"claudeAiOauth":{"accessToken":"aistat-partition-test","refreshToken":"","expiresAt":0}}`)
	if err := WriteClaudeLiveBlob(ctx, sentinel); err != nil {
		t.Fatalf("WriteClaudeLiveBlob: %v", err)
	}

	// Inspect the partition list via security find-generic-password -g.
	out, err := exec.CommandContext(ctx, "security", "find-generic-password",
		"-s", "Claude Code-credentials", "-g").CombinedOutput()
	if err != nil {
		t.Fatalf("security find-generic-password -g: %v\n%s", err, out)
	}
	outStr := string(out)
	if !strings.Contains(outStr, "apple-tool:") {
		t.Errorf("partition list does not contain apple-tool:; output:\n%s", outStr)
	}
	if !strings.Contains(outStr, "apple:") {
		t.Errorf("partition list does not contain apple:; output:\n%s", outStr)
	}
}
