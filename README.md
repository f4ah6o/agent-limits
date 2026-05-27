# usage-check

A single static **Go binary** that reports your Claude, Codex, and Copilot usage from the terminal.

```
$ usage-check -h
Claude usage
- 5-hour: 6.0% (resets in 4h 48m)
- 7-day: 42.0% (resets in 48m)
- 7-day sonnet: 10.0% (resets in 48m)

Codex usage
- 5-hour: 2.0% (resets in 2h 26m)
- 7-day: 0.0% (resets in 6d 21h)

Copilot usage
- month: 67.3% (resets in 5d 1h)
```

JSON is the default; `-h`/`--human` opts into the text rendering above.

## Install

Prebuilt binaries are available on the [Releases page](https://github.com/drogers0/llm-usage/releases).

For Go users, install from source:

```
go install github.com/drogers0/llm-usage/cmd/usage-check@latest
```

Requires Go 1.22+. Claude, Codex, and Copilot all work on macOS and Linux. The Claude provider reads from the macOS Keychain item populated by `claude /login`, or from `~/.claude/.credentials.json` on Linux.

## Usage

```
usage-check                # all services, JSON
usage-check claude         # claude only, JSON
usage-check codex          # codex only, JSON
usage-check copilot        # copilot only, JSON
usage-check -h             # all services, human-readable text
usage-check claude -h      # claude only, human-readable
usage-check --debug        # one line per HTTP request + per-provider summary, to stderr
usage-check --version      # print version and exit
usage-check --help         # print help
```

`-h` is the short form of `--human` (the text renderer). Help is `--help` only.

## How it works

Each provider has one credential source and one HTTPS endpoint:

| Provider | Credential | Endpoint |
|----------|------------|----------|
| Claude   | macOS Keychain item `Claude Code-credentials` (populated by `claude /login`) | `api.anthropic.com/api/oauth/usage` |
| Codex    | `~/.codex/auth.json` (populated by `codex login`) | `chatgpt.com/backend-api/wham/usage` |
| Copilot  | `gh auth token` (populated by `gh auth login`; needs the `user` scope) | `api.github.com/users/{login}/settings/billing/premium_request/usage` |

Providers are fetched in parallel. A failing provider does not block the others; the binary exits 0 only when every requested provider succeeded, otherwise exit 2. Each failed provider's error message is surfaced in the JSON (`providers.<id>.error`) and as `<Cap> usage: <error>` in text mode.

## Diagnostics on stderr

Even without `--debug`, the Copilot provider may emit one diagnostic line to stderr when it detects an API drift signal:

    usage-check: copilot: Copilot-product usageItems present but none matched
    sku="Copilot Premium Request" — GitHub may have renamed the SKU; please
    file an issue at https://github.com/drogers0/llm-usage/issues

The exit code and stdout payload are unaffected — this is a heads-up that the underlying number may be stale. With `--debug`, additional per-request and per-provider lines are also written to stderr.

## Authentication

If a provider's credential is missing, the error message names the exact command to fix it. For Copilot, the `user` scope is required:

```
gh auth refresh -h github.com -s user
```

If GitHub returns a Copilot plan slug `usage-check` doesn't recognize, the provider fails closed with a message naming the slug and a link to file an issue.

## Output contract

```json
{
  "checked_at": "2026-05-26T22:00:00+00:00",
  "providers": {
    "claude":  { "limits": { "five_hour": {"used_percent": 6, "remaining_percent": 94, "resets_at": "2026-05-27T03:00:00+00:00", "reset_after_seconds": 17280}, ... } },
    "codex":   { "limits": { ... } },
    "copilot": { "limits": { "month": { ... } } }
  }
}
```

Every `Limit` has the same four fields: `used_percent`, `remaining_percent`, `resets_at` (ISO 8601, always `+00:00` for UTC, never `Z`), `reset_after_seconds`. The top-level `providers` map is alphabetically sorted.

## v2 breaking changes (from v1.0.0)

- **JSON is now the default.** `-h`/`--human` opts into text. The old `--json` flag is removed (use bare `usage-check`).
- **Distribution:** Go binary via `go install`. The TypeScript + Chrome extension + native messaging host architecture is gone; the v1 sources are preserved at the [`v1.0.0` tag](https://github.com/drogers0/llm-usage/releases/tag/v1.0.0) and the `legacy/typescript-chrome-extension` branch.

## License

MIT — see [LICENSE](LICENSE).
