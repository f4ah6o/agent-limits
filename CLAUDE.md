# agent-usage

Rust CLI for reporting Claude Code, Codex, and OpenCode Go usage limits.

## CLI surface

```bash
agent-usage
agent-usage usage [claude|codex|opencodego]
agent-usage usage --refresh
agent-usage --human usage
agent-usage --debug usage <provider>
agent-usage opencodego setup
agent-usage opencodego setup --workspace-id <id> --auth-cookie <cookie>
```

JSON is the default. `--human` switches to text rendering.

## Repository layout

- `src/main.rs` and `src/cli/` contain clap dispatch.
- `src/providers/` contains provider-specific usage clients.
- `src/cred/` reads provider credentials. Do not print or log secret values.
- `src/render/` contains JSON and text renderers.
- `src/providers/usagecache.rs` owns the 90 second usage cache.

## Provider notes

- Claude reads the existing Claude Code credential. It does not implement login.
- Codex reads `~/.codex/auth.json`. It does not implement login.
- OpenCode Go reads `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE`, or `~/Library/Application Support/opencode-bar/opencode-go.json`.
- On macOS, `agent-usage opencodego setup` can extract the OpenCode Go workspace and auth cookie from the local Chrome profile without printing the cookie.

## Release

- Versioning is CalVer: `YYYY.M.PATCH`.
- First Rust-only release: `2026.6.0`.
- Release tags use `vYYYY.M.PATCH`.
- cargo-dist builds GitHub release artifacts.
- crates.io publishing should use Trusted Publishing.

## Checks

Run these before publishing:

```bash
cargo fmt --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
cargo publish --dry-run
dist generate --check
dist manifest --artifacts=all --output-format=json --no-local-paths
```

Live provider checks are optional because they depend on local credentials and upstream rate limits.
