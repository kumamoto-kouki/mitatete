// WebdriverIO + tauri-driver による Tauri アプリ E2E 設定（動作確認済み）。
//
// 前提（導入手順は docs/e2e-setup.md。この環境では導入済み・smoke PASS）:
//   - OS の `WebKitWebDriver`（Linux: apt の webkitgtk-webdriver）
//   - `cargo install tauri-driver`
//   - npm dev 依存: @wdio/cli @wdio/local-runner @wdio/mocha-framework webdriverio
//
// `pnpm e2e` で実行（cwd = リポジトリルート。onPrepare が本番ビルドを自動実行）。
//
// 解決した3つのハマりどころ:
//   1) tauri-driver は `--native-driver` を明示しないと WebKitWebDriver を見つけられない。
//   2) wdio v9 は既定で BiDi を要求するが WebKitWebDriver は classic のみ
//      → `wdio:enforceWebDriverClassic: true`。
//   3) `cargo build --release` 直叩きは dev サーバー(localhost:1420)を読みに行く
//      → `tauri build`（CLI）で本番フロント(frontendDist)を埋め込む。

import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { resolve } from "node:path";

let tauriDriver: ChildProcess | undefined;

// 環境変数で WebKitWebDriver のパスを上書き可能（既定は Linux の標準位置）。
const NATIVE_DRIVER =
  process.env.WEBKIT_WEBDRIVER ?? "/usr/bin/WebKitWebDriver";

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // tauri-driver が webkit2gtk のドライバへ橋渡しする。
      browserName: "wry",
      // wdio v9 は既定で BiDi(webSocketUrl) を要求するが WebKitWebDriver は classic のみ。
      // これを付けないと session 作成が "Failed to match capabilities" で落ちる。
      "wdio:enforceWebDriverClassic": true,
      "tauri:options": {
        // tauri-driver は絶対パスでアプリを起動する（相対だと native driver が解決できない）。
        application: resolve(
          process.cwd(),
          "src-tauri/target/release/mitatete"
        ),
      },
    } as WebdriverIO.Capabilities,
  ],
  hostname: "127.0.0.1",
  port: 4444,
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 120000 },

  // 本番フロント（frontendDist）を埋め込んだリリースバイナリを作る。
  // 注意: `cargo build --release` 直叩きだと dev サーバー(localhost:1420)を読みに行くため不可。
  // `tauri build`（CLI）で本番モードにする。--no-bundle で installer 生成は省略。
  onPrepare: () => {
    spawnSync("pnpm", ["tauri", "build", "--no-bundle"], { stdio: "inherit" });
  },
  beforeSession: async () => {
    // --native-driver を明示しないと WebKitWebDriver を見つけられず
    // "Failed to match capabilities" で session 作成に失敗する。
    tauriDriver = spawn("tauri-driver", ["--native-driver", NATIVE_DRIVER], {
      stdio: [null, process.stdout, process.stderr],
    });
    // tauri-driver と配下の WebKitWebDriver が bind するまで待つ（未準備だと
    // session 作成が "Failed to match capabilities" で落ちる）。
    await new Promise((r) => setTimeout(r, 4000));
  },
  afterSession: () => {
    tauriDriver?.kill();
  },
};
