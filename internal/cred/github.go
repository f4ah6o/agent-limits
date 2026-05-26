package cred

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
)

const GitHubTokenMissingMessage = "GitHub token unavailable — run `gh auth login`. If 'Not Found', run `gh auth refresh -h github.com -s user` to add the required scope."

var ErrGitHubTokenNotFound = errors.New(GitHubTokenMissingMessage)

// ReadGitHubToken shells out to `gh auth token`. Returns ErrGitHubTokenNotFound
// (the only sentinel callers should match against) if gh is not on PATH or has
// no token configured.
func ReadGitHubToken(ctx context.Context) (string, error) {
	cmd := exec.CommandContext(ctx, "gh", "auth", "token")
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		var ee *exec.ExitError
		var pe *exec.Error
		switch {
		case errors.As(err, &pe):
			return "", fmt.Errorf("%w: gh not on PATH (%s)", ErrGitHubTokenNotFound, pe.Error())
		case errors.As(err, &ee):
			return "", fmt.Errorf("%w: gh auth token failed: %s", ErrGitHubTokenNotFound, strings.TrimSpace(stderr.String()))
		}
		return "", fmt.Errorf("gh auth token failed: %w", err)
	}
	token := strings.TrimSpace(string(out))
	if token == "" {
		return "", ErrGitHubTokenNotFound
	}
	return token, nil
}
