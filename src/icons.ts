// icons.ts — Lucide アイコン初期化（ローカルバンドル、CDN 不使用）
// tree-shakeable な個別 import で必要なアイコンだけバンドルする。
// CSP 'script-src 'self'' に完全準拠（外部スクリプト不要）。

import {
  createIcons,
  Info,
  Bot,
  Send,
  ArrowUp,
  User,
  Sparkles,
  Check,
  BookOpen,
  Sun,
  Moon,
} from "lucide";

/**
 * data-lucide 属性を持つ要素を SVG に置換する。
 * DOMContentLoaded 後 + 動的生成要素の挿入後に呼ぶこと。
 */
export function initIcons(): void {
  createIcons({
    icons: {
      Info,
      Bot,
      Send,
      ArrowUp,
      User,
      Sparkles,
      Check,
      BookOpen,
      Sun,
      Moon,
    },
  });
}

// 静的 HTML（index.html）の data-lucide 要素を初回置換する。
// 動的生成要素（main.ts / model-ui.ts / character-ui.ts 等）は
// 各モジュールが initIcons() を再呼び出しして置換する。
if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initIcons);
  } else {
    initIcons();
  }
}
