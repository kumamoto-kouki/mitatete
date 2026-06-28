// テーマ（ライト/ダーク）の永続化ユーティリティ。
// Tauri の emit を含まない純粋モジュール — happy-dom テスト環境で import しても安全。
// emit は呼び出し元（main.ts）が担う。

export const THEME_STORAGE_KEY = "mitatete-theme";
export type Theme = "light" | "dark";

/** localStorage から読み込んだ値、または既定の "light" を返す */
export function loadTheme(): Theme {
  const stored = localStorage.getItem(THEME_STORAGE_KEY);
  return stored === "dark" ? "dark" : "light";
}

/** data-theme をセットし localStorage に保存する（emit は行わない） */
export function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute("data-theme", theme);
  localStorage.setItem(THEME_STORAGE_KEY, theme);
  const toggle = document.querySelector<HTMLButtonElement>("#theme-toggle");
  if (toggle) {
    toggle.setAttribute("aria-pressed", theme === "dark" ? "true" : "false");
  }
}
