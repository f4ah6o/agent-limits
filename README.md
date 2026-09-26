# agent-limits

`agent-limits` reports Claude Code, Codex, Devin CLI, and OpenCode Go usage limits from the terminal.

JSON is the default output. Pass `--human` for a compact text view or `--bar` for a visual bar graph.

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
agent-limits usage devin             # report Devin CLI only
agent-limits usage opencodego        # report OpenCode Go only, even when disabled
agent-limits usage --refresh         # bypass the 90 s usage cache
agent-limits --human usage           # human-readable output
agent-limits --debug usage codex     # request/debug lines on stderr
agent-limits usage opencodego --bar  # visual bar graph
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

### Bar graph (`--bar`)

Use `--bar` for a visual bar graph of usage per window:

```text
Opencodego usage
  5-hour   ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  0.0%  5h 0m
  7-day    █████████████░░░░░░░░░░░░░░░░░  42.0%  1d 21h
  Monthly  ███████████████████████░░░░░░░  78.0%  6d 4h
```

Bars are colour-coded: green (<30%), yellow (30–70%), red (>70%). Set `NO_COLOR=1` to disable colour.

## Authentication

| Provider | Setup |
|---|---|
| Claude | `claude /login` |
| Codex | `codex login` |
| Devin | `devin auth login` (reuses the Devin CLI credential under `~/.local/share/devin/credentials.toml`) |
| OpenCode Go | `agent-limits opencodego setup` on macOS Chrome, or set `OPENCODE_GO_WORKSPACE_ID` and `OPENCODE_GO_AUTH_COOKIE` |

`agent-limits opencodego setup` stores the workspace ID and auth cookie in the OpenCode Bar configuration file with owner-only permissions.

Devin support reads the Devin CLI credential without modifying it and reports the account-wide daily and weekly quota returned by Devin's user-status service. That service is not a public API, so Devin CLI updates may require a corresponding `agent-limits` update.

## Acknowledgements

- This project originated from [drogers0/aistat](https://github.com/drogers0/aistat).
- The OpenCode Go usage handling references [opgginc/opencode-bar](https://github.com/opgginc/opencode-bar).
- The Devin usage handling was cross-checked against [wakamex/devin-cli-usage](https://github.com/wakamex/devin-cli-usage) and current Devin CLI behavior.

## License

[MIT](LICENSE)
