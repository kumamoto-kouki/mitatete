# E2E（tauri-driver + WebdriverIO）セットアップ

Tauri アプリの End-to-End テスト。`e2e/wdio.conf.ts`・`e2e/app.e2e.ts` は **scaffold 済み**だが、
実行には OS レベルの前提が必要。当開発環境（WSLg）では `WebKitWebDriver`・`tauri-driver` が
未導入のため**未実行**（happy-dom の DOM テストと `cargo test` を主証拠とする縮退方針）。

## 有効化手順（実行する環境で1回）

1. **WebKitWebDriver**（Linux）:
   ```bash
   sudo apt-get install -y webkit2gtk-driver   # WebKitWebDriver を提供
   ```
2. **tauri-driver**:
   ```bash
   cargo install tauri-driver --locked
   ```
3. **npm dev 依存**（未インストール。コスト配慮で保留中）:
   ```bash
   pnpm add -D @wdio/cli @wdio/local-runner @wdio/mocha-framework webdriverio
   ```
4. **リリースビルド**（`wdio.conf.ts` の onPrepare でも自動実行）:
   ```bash
   pnpm tauri build   # または cargo build --release（src-tauri）
   ```
5. **package.json にスクリプト追加**:
   ```json
   "e2e": "wdio run e2e/wdio.conf.ts"
   ```
6. **実行**: `pnpm e2e`

## 状態

- ✅ 設定・spec を scaffold（`e2e/`）。
- ⏸ npm 依存・`tauri-driver`・`WebKitWebDriver` は環境準備後に導入（この環境では不可）。
- 検証自動化の主証拠は当面 happy-dom（DOM 単体）＋ `cargo test`＋ insta（スナップショット）。
