package accounts

import (
	"context"
	"io"
	"sync"
)

// Store persists Claude accounts across invocations. All methods are safe for
// concurrent use. The concrete implementation is platform-specific; obtain one
// via OpenStore.
type Store interface {
	// List returns all stored accounts in unspecified order.
	List(ctx context.Context) ([]Account, error)
	// Upsert inserts or replaces the account identified by a.UUID.
	Upsert(ctx context.Context, a Account) error
	// Delete removes the account with the given UUID. No-op if absent.
	Delete(ctx context.Context, uuid string) error
}

// config holds options passed to OpenStore.
type config struct {
	debug io.Writer // nil disables debug output
}

// Option configures OpenStore behaviour.
type Option func(*config)

// WithDebug wires a writer that receives debug/diagnostic lines (e.g. orphan
// index warnings on darwin). The writer is wrapped in a mutex-guarded adapter
// internally — callers may pass an unguarded *bytes.Buffer without worrying
// about goroutine races from concurrent List/Upsert/Delete calls. The wrap is
// unconditional; the per-call overhead is negligible for the orphan-warn path.
func WithDebug(w io.Writer) Option {
	return func(c *config) { c.debug = w }
}

// safeWriter wraps an io.Writer with a mutex. Mirrors httpx.ConcurrencySafeWriter.
// Needed because the orphan-warn path in darwin's List may be called from
// goroutines exercising the store concurrently (e.g. parallel test cases),
// allowing callers to pass a plain *bytes.Buffer without a data race.
type safeWriter struct {
	mu sync.Mutex
	w  io.Writer
}

func (s *safeWriter) Write(p []byte) (int, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.w.Write(p)
}
