# Store five-hour usage in a Cloudflare service

Status: polished
Model: GPT-5
Created: 2026-06-27
Updated: 2026-06-27
Branch: codex/20260627-cloudflare-usage-store

## 概要

`agent-limits usage` が取得した provider usage から `five_hour` ウィンドウだけを抽出し、GitHub login 済みユーザーごとの最新値として保存する Cloudflare Workers サービスを追加する。
サービスは GitHub OAuth で利用者を識別し、CLI は GitHub login 後に発行された ingestion token を使って `five_hour` usage を送信する。

## 背景

このリポジトリは Claude Code、Codex、OpenCode Go の利用制限を Rust CLI で報告する。
既存の出力はローカルで完結しており、`src/providers/mod.rs` の `Limit` は `used_percent`、`remaining_percent`、`resets_at`、`reset_after_seconds` を持つ。
Claude、Codex、OpenCode Go は `five_hour` キーを使って短期ウィンドウを表現している。

端末や実行環境をまたいで短期 usage を参照するには、provider credential ではなく、CLI がすでに取得した usage の最小データだけをクラウド側へ保存すればよい。
Cloudflare Workers を API 層にし、D1 を永続化先にする。
Cloudflare secret や 1Password Developer Environments を扱う場合は、secret 値を表示・記録せず、1Password MCP server の認可と runtime injection を使う。

## 問題

現在は `agent-limits usage` の結果をクラウドへ登録・保持する仕組みがない。
保存対象、認証境界、secret の扱いを先に固定しないと、provider credential、Cookie、アクセストークン、週次・月次 usage、debug log などの不要なデータを保存する実装になりやすい。

また、CLI が GitHub OAuth のフル login flow を直接持つと初期実装の範囲が広がる。
初期版では GitHub login は Cloudflare 側で完結させ、CLI は login 済みユーザーが発行した ingestion token を使う。

## 目標

GitHub login 済みユーザーごとに、Claude、Codex、OpenCode Go の `five_hour` usage 最新値を Cloudflare D1 に upsert できる。
CLI は provider usage report から `five_hour` だけを抽出し、provider ID、limit 値、取得時刻、CLI バージョンを送信する。
サーバーは token から GitHub user ID を解決し、同一ユーザー・同一 provider の最新値だけを保持する。

## 対象外

- Claude、Codex、OpenCode Go へのクラウド側ログイン代行
- provider credential、auth cookie、access token、refresh token の保存
- 週次・月次 usage の保存
- provider usage のクラウド側取得
- CLI 内での GitHub OAuth device flow または browser login
- 履歴保存、集計、チーム共有、課金、通知
- Cloudflare 以外のホスティング実装

## 提案する方針

Cloudflare Worker を `cloudflare/usage-store/` に追加する。
構成は TypeScript、`wrangler.jsonc`、D1 migration、Worker unit tests を含める。
Worker は最新の Cloudflare Workers docs と `wrangler types` で生成した `Env` 型に合わせ、binding は REST API ではなく D1 binding 経由で使う。
`compatibility_date`、`nodejs_compat`、observability、非 secret 設定は `wrangler.jsonc` に置き、GitHub OAuth secret、session secret、token pepper などの secret は Cloudflare secrets または 1Password runtime injection で渡す。

D1 schema は少なくとも次を持つ。

```text
users(github_user_id primary key, github_login, created_at, updated_at)
ingestion_tokens(id primary key, github_user_id, token_hash, created_at, last_used_at, revoked_at)
usage_latest(github_user_id, provider_id, used_percent, remaining_percent, resets_at, reset_after_seconds, checked_at, cli_version, updated_at, primary key(github_user_id, provider_id))
```

API は最小限にする。

- `GET /auth/github/start` は GitHub OAuth を開始する。
- `GET /auth/github/callback` は GitHub user ID を確定し、httpOnly secure session cookie を発行する。
- `POST /api/tokens` は login 済みユーザーに ingestion token を一度だけ表示する。保存するのは hash だけにする。
- `POST /api/usage` は `Authorization: Bearer <token>` を必須にし、`provider_id` と `five_hour` limit payload を検証して upsert する。
- `GET /api/usage` は login 済みセッションまたは bearer token の所有者に、自分の最新 usage だけを返す。

Rust CLI にはクラウド送信用の狭いサブコマンドを追加する。
コマンド名は `agent-limits cloud push` とし、既存の `usage` 出力形式は変えない。
`agent-limits cloud push [claude|codex|opencodego]` は既存の provider fetch 経路を再利用し、`five_hour` が存在する provider だけを送信する。
送信先 URL は `AGENT_LIMITS_CLOUD_URL` または設定ファイルから読み、ingestion token は `AGENT_LIMITS_CLOUD_TOKEN` または owner-only 権限の設定ファイルから読む。
`--debug` 時も token、provider credential、cookie は出力しない。

payload は次の形に限定する。

```json
{
  "provider_id": "claude",
  "checked_at": "2026-06-27T00:00:00+00:00",
  "cli_version": "2026.6.0",
  "five_hour": {
    "used_percent": 12.34,
    "remaining_percent": 87.66,
    "resets_at": "2026-06-27T05:00:00+00:00",
    "reset_after_seconds": 18000
  }
}
```

## 受け入れ条件

- [ ] Given a GitHub-authenticated session, when the user creates an ingestion token, then the plaintext token is shown once and only its hash is stored in D1.
- [ ] Given a valid ingestion token, when `POST /api/usage` receives a supported `provider_id` and `five_hour` payload, then D1 upserts the latest row for that GitHub user and provider.
- [ ] Given the same user and provider already have a row, when a newer valid payload is posted, then the existing row is updated instead of creating a duplicate.
- [ ] Given an unauthenticated request or invalid bearer token, when `/api/usage` is called, then the Worker returns 401 and writes no usage row.
- [ ] Given user A and user B both have usage rows, when either user reads `/api/usage`, then only that user's rows are returned.
- [ ] Given a provider report contains `seven_day` or `monthly`, when `agent-limits cloud push` builds the request, then those windows are omitted from the payload.
- [ ] Given a provider report has no `five_hour` window, when `agent-limits cloud push` runs, then that provider is skipped with a non-secret diagnostic and no malformed payload is sent.
- [ ] Given normal and debug logging paths, when requests fail or succeed, then provider credentials, auth cookies, access tokens, refresh tokens, ingestion tokens, and Cloudflare secret values are not logged.
- [ ] Given local development setup, when a developer follows the README or operations docs, then they can create the D1 database, apply migrations, configure GitHub OAuth secrets, run the Worker locally, and run the Rust validation commands.

## テスト計画

- Rust unit tests for `agent-limits cloud push` payload construction:
  - includes only `five_hour`;
  - preserves `used_percent`, `remaining_percent`, `resets_at`, `reset_after_seconds`;
  - omits `seven_day`, `seven_day_sonnet`, and `monthly`;
  - skips providers without `five_hour`.
- Rust CLI tests or focused unit tests for cloud config loading:
  - reads URL and token from environment;
  - refuses missing URL/token with non-secret errors;
  - redacts token-like values in debug/error output.
- Worker unit/integration tests with a local D1 test database:
  - GitHub callback creates/updates a user;
  - token creation stores hash only;
  - valid `POST /api/usage` upserts;
  - invalid auth returns 401;
  - cross-user reads are rejected or filtered;
  - schema validation rejects unsupported providers and extra usage windows.
- Manual local Worker check:
  - run `wrangler d1 migrations apply --local`;
  - run the Worker locally;
  - complete GitHub OAuth against a local callback or documented development callback;
  - create an ingestion token;
  - run `agent-limits cloud push opencodego` against the local Worker with test credentials.
- Repository checks:
  - `cargo fmt --check`
  - `cargo check --all-targets`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - Worker typecheck/test commands added by the implementation, for example `npm test` and `npx tsc --noEmit` from `cloudflare/usage-store/`
  - release-readiness checks only if the implementation changes package metadata or release artifacts.

## リスク

Adding a Worker introduces a second toolchain and deployment surface in a Rust CLI repository.
Keep the Cloudflare project self-contained under `cloudflare/usage-store/` and document the exact commands needed for local development.

GitHub OAuth and ingestion tokens introduce new secrets.
Do not print token values in logs, test snapshots, panic messages, or setup output.
Use cryptographically strong token generation and compare token hashes rather than storing plaintext.

Provider windows are normalized by string keys.
If a provider changes its five-hour key away from `five_hour`, `cloud push` must skip it rather than guessing from labels or reset durations.

The repository documentation currently mentions both `agent-usage` and the actual binary/crate name `agent-limits`.
This issue should implement against the current Rust binary name `agent-limits`; any separate rename/alignment should be handled in another issue.

## 変更履歴

`CHANGES.md` impact: yes.

Suggested entry:

- Add a Cloudflare-backed usage store and `agent-limits cloud push` for uploading only five-hour usage with GitHub-authenticated ownership.

## 注記

`issues/polished/` is introduced by this issue refinement because the local issue validator requires the state directory to match `Status: polished`.
If 1Password Developer Environments are used for Cloudflare or GitHub OAuth secrets, use the 1Password MCP server and runtime injection. Do not ask to print or paste secret values.
