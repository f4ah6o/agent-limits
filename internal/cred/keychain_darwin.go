//go:build darwin

package cred

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os/exec"
	"strings"
)

const ClaudeTokenMissingMessage = "claude token not found in Keychain — run `claude /login` to authenticate"

var ErrClaudeTokenNotFound = errors.New(ClaudeTokenMissingMessage)

// ReadClaudeToken returns the OAuth access token from the macOS Keychain item
// "Claude Code-credentials". Triggers a system prompt the first time per
// binary (or after a code-signing change) — unavoidable for non-interactive
// keychain reads without registering as a trusted app.
func ReadClaudeToken(ctx context.Context) (string, error) {
	cmd := exec.CommandContext(ctx, "security", "find-generic-password",
		"-s", "Claude Code-credentials", "-w")
	out, err := cmd.Output()
	if err != nil {
		var ee *exec.ExitError
		if errors.As(err, &ee) {
			stderr := strings.TrimSpace(string(ee.Stderr))
			if strings.Contains(stderr, "could not be found") {
				return "", ErrClaudeTokenNotFound
			}
			return "", fmt.Errorf("keychain access failed: %s", stderr)
		}
		return "", fmt.Errorf("keychain access failed: %w", err)
	}
	var cred struct {
		ClaudeAiOauth struct {
			AccessToken string `json:"accessToken"`
		} `json:"claudeAiOauth"`
	}
	if err := json.Unmarshal(out, &cred); err != nil {
		return "", fmt.Errorf("keychain value is not JSON: %w", err)
	}
	if cred.ClaudeAiOauth.AccessToken == "" {
		return "", ErrClaudeTokenNotFound
	}
	return cred.ClaudeAiOauth.AccessToken, nil
}
