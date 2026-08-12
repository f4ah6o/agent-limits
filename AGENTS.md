# agent-limits

Rust CLI for reporting Claude Code, Codex, and OpenCode Go usage limits.

README.md is for users. Keep implementation details, release procedures, repository structure, and maintainer-only notes in AGENTS.md or `docs/`.

## CLI surface

```bash
agent-limits
agent-limits usage [claude|codex|opencodego]
agent-limits usage --refresh
agent-limits --human usage
agent-limits --debug usage <provider>
agent-limits config list
agent-limits config enable <provider>
agent-limits config disable <provider>
agent-limits opencodego setup
agent-limits opencodego setup --workspace-id <id> --auth-cookie <cookie>
```

JSON is the default. `--human` switches to text rendering.

The default aggregate usage command queries only enabled providers. An explicitly named provider is queried even when disabled in configuration.

## Repository layout

- `src/main.rs` and `src/cli/` contain clap dispatch.
- `src/config.rs` loads and saves enabled provider state.
- `src/providers/` contains provider-specific usage clients.
- `src/cred/` reads provider credentials. Do not print or log secret values.
- `src/render/` contains JSON and text renderers.
- `src/providers/usagecache.rs` owns the 90 second usage cache.

## Provider configuration

Provider state is stored as JSON below the platform configuration directory in `agent-limits/config.json`.

- Missing configuration means every known provider is enabled.
- `config enable` and `config disable` are the supported mutation interface.
- `config list` prints all known provider states and the resolved config path.
- Validate provider IDs against `KNOWN_PROVIDER_IDS` before writing.
- Preserve explicit `usage <provider>` behavior independently of aggregate filtering.

## Provider notes

- Claude reads the existing Claude Code credential. It does not implement login.
- Codex reads `~/.codex/auth.json`. It does not implement login.
- OpenCode Go reads `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE`, or the OpenCode Bar configuration file.
- On macOS, `agent-limits opencodego setup` can extract the OpenCode Go workspace and auth cookie from the local Chrome profile without printing the cookie.

## Release

- Versioning is CalVer: `YYYY.M.PATCH`.
- First Rust-only release: `2026.6.0`.
- Release tags use `vYYYY.M.PATCH`.
- Move the mutable `latest` tag to a commit in `main` history to start a release.
- `publish.yml` calls `f4ah6o/calver-action` to allocate the CalVer, update Cargo metadata in a release-only commit, publish through crates.io Trusted Publishing, and create the immutable version tag.
- The caller then dispatches cargo-dist for that immutable tag so GitHub release artifacts are built from the same release-only commit.
- cargo-binstall uses the cargo-dist release archives through `[package.metadata.binstall]` in `Cargo.toml`.

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
