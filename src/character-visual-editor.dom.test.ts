// @vitest-environment happy-dom
// happy-dom 導入の疎通確認も兼ねた DOM 描画テスト。
// これまで「手動確認のみ」だった initVisualEditor（Tauri 非依存）の描画を自動検証する。

import { describe, it, expect } from "vitest";
import {
  initVisualEditor,
  DEFAULT_TEMPLATE_PARAMS,
} from "./character-visual-editor";

describe("initVisualEditor — DOM 描画（happy-dom）", () => {
  it("container にプレビュー img と各レイヤーの select を描画し、現在の VisualConfig を返す", () => {
    const container = document.createElement("div");
    const getConfig = initVisualEditor(container);

    // プレビュー img（data URI）と 3 つの select（体型/目/髪）が描画される。
    const img = container.querySelector("img.visual-editor__preview");
    expect(img).not.toBeNull();
    expect(img?.getAttribute("src") ?? "").toContain("data:image/svg+xml");
    expect(container.querySelectorAll("select").length).toBe(3);
    expect(container.querySelectorAll('input[type="color"]').length).toBe(2);

    // getConfig は template の VisualConfig を返す。
    const config = getConfig();
    expect(config.mode).toBe("template");
    expect(config.templateParams?.bodyType).toBe(
      DEFAULT_TEMPLATE_PARAMS.bodyType
    );
  });
});
