// WebdriverIO + tauri-driver による Tauri アプリ E2E 設定（scaffold）。
//
// 実行前提（この環境では未整備のため未実行。docs/e2e-setup.md 参照）:
//   - `cargo install tauri-driver`
//   - OS の `WebKitWebDriver`（Linux: apt の webkit2gtk-driver 等）
//   - リリースビルド済みアプリ `src-tauri/target/release/mitatete`
//   - npm dev 依存: @wdio/cli @wdio/local-runner @wdio/mocha-framework webdriverio
//
// 有効化したら `pnpm e2e` で実行する。

import { spawn, spawnSync, type ChildProcess } from "node:child_process";

let tauriDriver: ChildProcess | undefined;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // tauri-driver が webkit2gtk のドライバへ橋渡しする。
      browserName: "wry",
      "tauri:options": {
        application: "../src-tauri/target/release/mitatete",
      },
    } as WebdriverIO.Capabilities,
  ],
  hostname: "127.0.0.1",
  port: 4444,
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 60000 },

  // リリースビルドを保証してから tauri-driver を起動する。
  onPrepare: () => {
    spawnSync("cargo", ["build", "--release"], {
      cwd: "../src-tauri",
      stdio: "inherit",
    });
  },
  beforeSession: () => {
    tauriDriver = spawn("tauri-driver", [], { stdio: [null, process.stdout, process.stderr] });
  },
  afterSession: () => {
    tauriDriver?.kill();
  },
};
