import { defineConfig, configDefaults } from "vitest/config";

// Tauri v2 + vanilla TypeScript（フレームワークなし）。
// main（チャットUI）と character（透明ウィンドウ）の 2 ページ構成。
export default defineConfig({
  // Tauri CLI の出力を消さない
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    // webview 互換ターゲット
    target: "es2022",
    rollupOptions: {
      input: {
        main: "index.html",
        character: "character.html",
      },
    },
  },
  test: {
    // 委譲エージェントの worktree（.claude/worktrees/**）を走査しない。
    // 残置 worktree のテストが二重カウントされる事故を防ぐ（実運用の教訓）。
    exclude: [...configDefaults.exclude, "**/.claude/**", "e2e/**"],
  },
});
