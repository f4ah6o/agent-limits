# Rust依存グラフを縮小する

Status: open
Model: GPT-5.6 Sol
Created: 2026-08-12
Updated: 2026-08-12
Branch: docs/20260812-dependency-reduction
Priority: P1

## 概要

`agent-limits` の直接依存と推移依存を見直し、機能と互換性を維持したまま、通常ビルドに不要なcrateを依存グラフから外す。

最初にOS固有機能の依存境界を修正し、その後に未使用依存とfeatureを整理する。

## 背景

現在の `Cargo.toml` では、macOS専用のChromium cookie取得・復号で利用する `rusqlite`、`aes`、`cbc`、`pbkdf2`、`sha1` などが全platform共通dependencyとして宣言されている。
一方、該当実装は `src/cred/chrome_cookie_darwin.rs` で、module自体は `#[cfg(target_os = "macos")]` によりmacOSだけで有効になる。

このためLinux向けビルドでも、実行時に利用しないSQLite・暗号関連crateを解決・コンパイルする可能性がある。

## 目標

- OS固有機能の依存を対応するtarget dependencyへ移す。
- 未使用の直接依存を削除する。
- 不要なdefault featureを無効化する。
- CLIの既存挙動、credential探索、provider取得、出力形式を変えない。
- 変更前後の依存package数とrelease binary sizeを記録する。

## 提案する方針

### 1. macOS専用dependencyを分離する

少なくとも次のcrateの使用箇所を確認し、macOS専用であれば `[target.'cfg(target_os = "macos")'.dependencies]` へ移す。

- `rusqlite`
- `aes`
- `cbc`
- `pbkdf2`
- `sha1`

`dirs` など共通コードでも利用されるcrateは、実使用箇所に応じて残す。

### 2. 未使用依存を検査する

`cargo machete` 相当のunused dependency検査を行い、特に `hmac` を含む直接依存が実コードから参照されているか確認する。
未使用であれば削除する。

### 3. featureを最小化する

主要crateについて `cargo tree --edges features` を確認し、不要なdefault featureが有効になっている場合は `default-features = false` と明示featureへ切り替える。
ただしTLS、JSON、CLI helpなど既存動作に必要なfeatureは維持する。

### 4. 依存回帰を計測する

変更前後で少なくとも次を比較する。

- `cargo tree` のpackage数
- `cargo tree -d` の重複package
- Linux release buildのbinary size
- macOS release buildのbinary size

## 受け入れ条件

- [ ] macOS専用実装だけが利用する直接依存はmacOS target dependencyへ移されている。
- [ ] Linux buildの依存グラフに、macOS Chromium cookie復号専用のSQLite・暗号crateが含まれない。
- [ ] 未使用の直接依存が削除されている。
- [ ] `cargo tree -d` とfeature graphを確認し、不要な重複・featureを削減または理由付きで残している。
- [ ] Claude、Codex、OpenCode Goの既存credential取得経路が維持される。
- [ ] macOSのChromium cookie取得が維持される。
- [ ] `cargo fmt --check` が成功する。
- [ ] `cargo check --all-targets` が成功する。
- [ ] `cargo test` が成功する。
- [ ] `cargo clippy --all-targets -- -D warnings` が成功する。
- [ ] 変更前後の依存package数とrelease binary sizeがPRまたはIssueの進捗欄に記録される。

## 対象外

- provider APIや認証仕様の変更
- credential保存形式の変更
- CLI commandや出力schemaの変更
- 暗号アルゴリズム自体の変更
- dependency更新を目的とした大規模version bump

## 実装順

1. baselineの依存package数・重複・binary sizeを計測する。
2. macOS専用依存をtarget dependencyへ移す。
3. unused dependencyを削除する。
4. default featureを精査する。
5. 全platform相当のCIと既存テストを通す。
6. before/afterを記録する。
