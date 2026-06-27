# Testing

Use the Rust toolchain only.

```bash
cargo fmt --check
cargo check --all-targets
cargo test
cargo clippy --all-targets -- -D warnings
```

Release-readiness checks:

```bash
cargo publish --dry-run
dist generate --check
dist manifest --artifacts=all --output-format=json --no-local-paths
```

Provider smoke tests:

```bash
target/debug/agent-usage --help
target/debug/agent-usage --human usage opencodego
```

Claude and Codex live checks depend on local credentials and upstream usage endpoint availability.
