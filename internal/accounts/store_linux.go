//go:build linux

package accounts

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"syscall"
)

type linuxStore struct {
	path string
}

// OpenStore returns the Linux file-backed account store at
// ~/.config/aistat/accounts/claude.json. The parent directory is created with
// mode 0700 if absent.
func OpenStore(opts ...Option) (Store, error) {
	// opts are accepted for API consistency; WithDebug is darwin-only.
	home, err := os.UserHomeDir()
	if err != nil {
		return nil, fmt.Errorf("accounts: cannot resolve home directory: %w", err)
	}
	dir := filepath.Join(home, ".config", "aistat", "accounts")
	if err := os.MkdirAll(dir, 0700); err != nil {
		return nil, fmt.Errorf("accounts: cannot create accounts dir %s: %w", dir, err)
	}
	return &linuxStore{path: filepath.Join(dir, "claude.json")}, nil
}

// withWriteLock opens the store file (creating it mode 0600 if absent),
// acquires LOCK_EX, calls fn with the open file, then releases the lock.
// Each call opens the file independently (new open file description), so
// concurrent goroutines also serialize correctly — flock on Linux is per OFD.
func (s *linuxStore) withWriteLock(fn func(f *os.File) error) error {
	f, err := os.OpenFile(s.path, os.O_RDWR|os.O_CREATE, 0600)
	if err != nil {
		return fmt.Errorf("accounts: open store file: %w", err)
	}
	defer f.Close()
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX); err != nil {
		return fmt.Errorf("accounts: acquire store lock: %w", err)
	}
	defer syscall.Flock(int(f.Fd()), syscall.LOCK_UN) //nolint:errcheck
	return fn(f)
}

// readAccountMap reads the current account map from f (must be at position 0).
// Returns an empty map (not an error) for an empty file.
// Returns an error for corrupt JSON so the caller can surface it rather than
// silently discarding stored accounts.
func readAccountMap(f *os.File) (map[string]Account, error) {
	if _, err := f.Seek(0, io.SeekStart); err != nil {
		return nil, fmt.Errorf("accounts: seek store file: %w", err)
	}
	data, err := io.ReadAll(f)
	if err != nil {
		return nil, fmt.Errorf("accounts: read store file: %w", err)
	}
	if len(data) == 0 {
		return make(map[string]Account), nil
	}
	var m map[string]Account
	if err := json.Unmarshal(data, &m); err != nil {
		return nil, fmt.Errorf("accounts: parse store file: %w", err)
	}
	return m, nil
}

// atomicWrite marshals m to JSON and atomically replaces s.path via a
// temp-file-in-same-dir + rename, preserving mode 0600.
func (s *linuxStore) atomicWrite(m map[string]Account) error {
	data, err := json.Marshal(m)
	if err != nil {
		return err
	}
	dir := filepath.Dir(s.path)
	tmp, err := os.CreateTemp(dir, ".claude-*.json.tmp")
	if err != nil {
		return fmt.Errorf("accounts: create temp file: %w", err)
	}
	tmpName := tmp.Name()
	var writeErr error
	if _, writeErr = tmp.Write(data); writeErr == nil {
		writeErr = tmp.Sync()
	}
	tmp.Close()
	if writeErr != nil {
		os.Remove(tmpName)
		return fmt.Errorf("accounts: write temp file: %w", writeErr)
	}
	// os.CreateTemp creates with mode 0600 — no chmod needed.
	if err := os.Rename(tmpName, s.path); err != nil {
		os.Remove(tmpName)
		return fmt.Errorf("accounts: rename temp file: %w", err)
	}
	return nil
}

func (s *linuxStore) List(ctx context.Context) ([]Account, error) {
	f, err := os.OpenFile(s.path, os.O_RDONLY, 0)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil, nil
		}
		return nil, fmt.Errorf("accounts: open store: %w", err)
	}
	defer f.Close()
	if err := syscall.Flock(int(f.Fd()), syscall.LOCK_SH); err != nil {
		return nil, fmt.Errorf("accounts: acquire store lock: %w", err)
	}
	defer syscall.Flock(int(f.Fd()), syscall.LOCK_UN) //nolint:errcheck

	m, err := readAccountMap(f)
	if err != nil {
		return nil, err
	}
	list := make([]Account, 0, len(m))
	for _, a := range m {
		list = append(list, a)
	}
	return list, nil
}

func (s *linuxStore) Upsert(ctx context.Context, a Account) error {
	return s.withWriteLock(func(f *os.File) error {
		m, err := readAccountMap(f)
		if err != nil {
			return err
		}
		m[a.UUID] = a
		return s.atomicWrite(m)
	})
}

func (s *linuxStore) Delete(ctx context.Context, uuid string) error {
	return s.withWriteLock(func(f *os.File) error {
		m, err := readAccountMap(f)
		if err != nil {
			return err
		}
		delete(m, uuid)
		if len(m) == 0 {
			// Remove the file entirely when the last account is deleted.
			return os.Remove(s.path)
		}
		return s.atomicWrite(m)
	})
}
