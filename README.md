# agent-limits

`agent-limits` reports Claude Code, Codex, and OpenCode Go usage limits from the terminal.

JSON is the default output. Pass `--human` for a compact text view.

## Install

From crates.io:

```bash
cargo install agent-limits
```

From GitHub release binaries with cargo-binstall:

```bash
cargo binstall agent-limits
```

## Usage

```bash
agent-limits                         # same as `agent-limits usage`
agent-limits usage                   # report all enabled providers
agent-limits usage claude            # report Claude only
agent-limits usage codex             # report Codex only
agent-limits usage opencodego        # report OpenCode Go only, even when disabled
agent-limits usage --refresh         # bypass the 90 s usage cache
agent-limits --human usage           # human-readable output
agent-limits --debug usage codex     # request/debug lines on stderr
```

Manage which providers are included in the default report:

```bash
agent-limits config list
agent-limits config disable opencodego
agent-limits config enable opencodego
```

Disabling a provider affects only `agent-limits` and `agent-limits usage`. Explicitly naming a provider still queries it.

Example:

```text
Opencodego usage
- 5-hour: 0.0% (resets in 5h 0m)
- 7-day: 53.0% (resets in 1d 21h)
- Monthly: 100.0% (resets in 6d 4h)
```

## Authentication

| Provider | Setup |
|---|---|
| Claude | `claude /login` |
| Codex | `codex login` |
| OpenCode Go | `agent-limits opencodego setup` on macOS Chrome, or set `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE` |

`agent-limits opencodego setup` stores the workspace ID and auth cookie in the OpenCode Bar configuration file with owner-only permissions.

## Acknowledgements

- This project originated from [drogers0/aistat](https://github.com/drogers0/aistat).
- The OpenCode Go usage handling references [opgginc/opencode-bar](https://github.com/opgginc/opencode-bar).

## License

[MIT](LICENSE)
