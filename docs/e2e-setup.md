# E2E（tauri-driver + WebdriverIO）セットアップ

Tauri アプリの End-to-End テスト。**この環境では導入済み・smoke PASS**（`pnpm e2e` → 1 passed）。
`e2e/wdio.conf.ts`・`e2e/app.e2e.ts` で実アプリを起動し、主要 UI の存在を検証する。

## 前提（実行環境で1回）

1. **WebKitWebDriver**（Linux）:
   ```bash
   sudo apt-get install -y webkitgtk-webdriver   # WebKitWebDriver を提供（旧 webkit2gtk-driver）
   ```
2. **tauri-driver**:
   ```bash
   cargo install tauri-driver --locked
   ```
3. **npm dev 依存**（導入済み）:
   ```bash
   pnpm add -D @wdio/cli @wdio/local-runner @wdio/mocha-framework webdriverio
   ```

## 実行

```bash
pnpm e2e
```

- `onPrepare` が `pnpm tauri build --no-bundle` で**本番フロントを埋め込んだ**リリースバイナリを作る。
- `beforeSession` が `tauri-driver --native-driver /usr/bin/WebKitWebDriver` を起動。
- `WEBKIT_WEBDRIVER` 環境変数で WebKitWebDriver のパスを上書き可能。

## ハマりどころ（解決済み・次世代向け）

1. **`--native-driver` 必須**: tauri-driver は WebKitWebDriver のパスを明示しないと
   `session not created: Failed to match capabilities` で落ちる。
2. **wdio v9 は BiDi 既定**: WebKitWebDriver は classic のみ対応のため
   `wdio:enforceWebDriverClassic: true` を capability に付ける（同じく capabilities mismatch）。
3. **`cargo build --release` 直叩きは不可**: dev サーバー(`http://localhost:1420`)を読みに行き
   「Connection refused」になる。`tauri build`（CLI）で本番モード＝`frontendDist` 埋め込みにする。
4. WSLg では libEGL/MESA 警告が出るがソフトレンダにフォールバックして動作する（無害）。

## 状態

- ✅ happy-dom（DOM 単体）＋ insta（Rust スナップショット）＋ **tauri-driver E2E（実アプリ smoke）** すべて稼働。
- E2E smoke は `#input`・`#model-panel`・`#character-panel`・`.chat__disclosure` の存在を確認。
