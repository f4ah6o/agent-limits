# aistat

A single static Go binary that reports Claude, Codex, and Copilot usage limits from the terminal. JSON by default; `-h`/`--human` for text output.

## Repo layout

- [cmd/aistat/](cmd/aistat/) — `main`, flag parsing, provider registry, fake-provider hooks for coverage tests.
- [internal/providers/](internal/providers/) — one subpackage per provider (`claude`, `codex`, `copilot`). Each owns its credential source, HTTP call, and response normalization into the shared `Limit` type in [types.go](internal/providers/types.go).
- [internal/render/](internal/render/) — `json` and `text` renderers. The JSON shape is the public contract; the text renderer is a thin presentation layer over the same data.
- [internal/cred/](internal/cred/), [internal/httpx/](internal/httpx/) — small shared helpers for credential lookup and HTTP transport.
- [internal/orchestrate/](internal/orchestrate/) — parallel fan-out across providers; one failing provider does not block the others.
- [internal/testutil/](internal/testutil/) — shared test helpers.

## Design principles

These are the principles every change should respect. When in doubt, optimize for the next reader.

### Simple

- One credential source and one HTTPS endpoint per provider. No fallbacks, no probing, no auto-discovery.
- No feature flags, no compatibility shims, no dead branches "in case." Delete code that isn't used.
- No catches for convoluted error states that are unlikely to be reached

### Robust

- Fail closed with an actionable message — the error names the next command the user should run to recover.
- One failing component never poisons another. Record its error in-band and keep the rest of the work going.

### Maintainable

- Each package reads end-to-end without jumping files. Names describe the domain, not the implementation.
- Comments are reserved for the non-obvious *why* — a vendor quirk, an invariant, a workaround. Don't restate what the code says.

### Elegant

- One source of truth per concept. Derived views (renderers, formatters) are pure functions of the model, never parallel implementations.
- Prefer the standard library. Reach for a third-party dependency only when the alternative is materially worse.

## Working in this repo

- Run `go test ./...` before declaring a change done. Use `go vet ./...` and `staticcheck` (pinned in CI to `2025.1.1`) for static checks.
- The Claude provider is macOS/Linux-only (Keychain on macOS, `~/.claude/.credentials.json` on Linux). Codex and Copilot are portable.
- The module path is `github.com/drogers0/aistat/v2`. Go 1.22+.
