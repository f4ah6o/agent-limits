# agent-limits

Rust CLI for reporting Claude Code, Codex, and OpenCode Go usage limits.

## CLI surface

```bash
agent-limits
agent-limits usage [claude|codex|opencodego]
agent-limits usage --refresh
agent-limits --human usage
agent-limits --debug usage <provider>
agent-limits opencodego setup
agent-limits opencodego setup --workspace-id <id> --auth-cookie <cookie>
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
- On macOS, `agent-limits opencodego setup` can extract the OpenCode Go workspace and auth cookie from the local Chrome profile without printing the cookie.

## Release

- Versioning is CalVer: `YYYY.M.PATCH`.
- First Rust-only release: `2026.6.0`.
- Release tags use `vYYYY.M.PATCH`.
- cargo-dist builds GitHub release artifacts.
- cargo-binstall uses the cargo-dist release archives through `[package.metadata.binstall]` in `Cargo.toml`.
- crates.io publishing should use Trusted Publishing.

Trusted Publishing settings for crates.io:

```text
Publisher: GitHub Actions
Repository owner: f4ah6o
Repository name: agent-limits
Workflow filename: publish.yml
Environment name: <empty>
```

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
