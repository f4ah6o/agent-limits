//go:build !darwin

package cred

import (
	"context"
	"errors"
)

const ClaudeTokenMissingMessage = "claude token not found in Keychain — run `claude /login` to authenticate"

var ErrClaudeTokenNotFound = errors.New(ClaudeTokenMissingMessage)

func ReadClaudeToken(ctx context.Context) (string, error) {
	return "", errors.New("claude provider is macOS-only (Keychain access required)")
}
