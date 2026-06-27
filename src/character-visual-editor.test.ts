import { describe, it, expect, vi } from "vitest";
import {
  buildTemplateVisualConfig,
  buildVisualSvg,
  svgToDataUri,
  requestImageUpload,
  COPYRIGHT_NOTICE,
  DEFAULT_TEMPLATE_PARAMS,
} from "./character-visual-editor";

describe("buildTemplateVisualConfig (要件 6.2)", () => {
  it("mode='template' と templateParams を持つ VisualConfig を生成する", () => {
    const config = buildTemplateVisualConfig(DEFAULT_TEMPLATE_PARAMS);
    expect(config.mode).toBe("template");
    expect(config.templateParams).toEqual(DEFAULT_TEMPLATE_PARAMS);
  });

  it("入力パラメータをコピーして格納する（参照を共有しない）", () => {
    const params = { ...DEFAULT_TEMPLATE_PARAMS };
    const config = buildTemplateVisualConfig(params);
    params.skinColor = "#000000";
    expect(config.templateParams?.skinColor).toBe(
      DEFAULT_TEMPLATE_PARAMS.skinColor
    );
  });
});

describe("buildVisualSvg (要件 6.1)", () => {
  it("肌色・服の色をSVGに反映する", () => {
    const svg = buildVisualSvg({
      ...DEFAULT_TEMPLATE_PARAMS,
      skinColor: "#123456",
      outfitColor: "#abcdef",
    });
    expect(svg).toContain("#123456");
    expect(svg).toContain("#abcdef");
    expect(svg.startsWith("<svg")).toBe(true);
  });

  it("体型がプレビューに反映される（data-body 属性）", () => {
    const svg = buildVisualSvg({
      ...DEFAULT_TEMPLATE_PARAMS,
      bodyType: "abstract",
    });
    expect(svg).toContain('data-body="abstract"');
  });

  it("髪 none のとき髪レイヤーを描かない（short とは異なる出力）", () => {
    const none = buildVisualSvg({
      ...DEFAULT_TEMPLATE_PARAMS,
      hairStyle: "none",
    });
    const short = buildVisualSvg({
      ...DEFAULT_TEMPLATE_PARAMS,
      hairStyle: "short",
    });
    expect(none).not.toBe(short);
  });

  it("svgToDataUri は data URI を返す", () => {
    const uri = svgToDataUri("<svg></svg>");
    expect(uri.startsWith("data:image/svg+xml;utf8,")).toBe(true);
  });
});

describe("requestImageUpload — 著作権同意フロー (要件 6.3, 6.4, 6.5)", () => {
  const png = new File([new Uint8Array([1, 2])], "me.png", {
    type: "image/png",
  });

  it("PNG + 同意で mode='upload' の VisualConfig を返す", async () => {
    const consent = vi.fn().mockResolvedValue(true);
    const config = await requestImageUpload(png, consent);
    expect(consent).toHaveBeenCalledWith(COPYRIGHT_NOTICE);
    expect(config).toEqual({ mode: "upload", uploadedImagePath: "me.png" });
  });

  it("SVG も受け付ける", async () => {
    const svg = new File(["<svg/>"], "art.svg", { type: "image/svg+xml" });
    const config = await requestImageUpload(svg, () => true);
    expect(config?.mode).toBe("upload");
  });

  it("同意拒否時はアップロードをキャンセルし既存設定を維持する (要件 6.4)", async () => {
    const existing = { mode: "template" as const };
    const config = await requestImageUpload(png, () => false, existing);
    expect(config).toBe(existing);
  });

  it("PNG/SVG 以外は受け付けず既存設定を維持する (要件 6.3)", async () => {
    const jpg = new File([new Uint8Array([1])], "x.jpg", {
      type: "image/jpeg",
    });
    const consent = vi.fn();
    const existing = { mode: "template" as const };
    const config = await requestImageUpload(jpg, consent, existing);
    expect(consent).not.toHaveBeenCalled(); // 形式不一致なら同意確認すらしない
    expect(config).toBe(existing);
  });

  it("パス解決を差し替えられる（Tauri dialog 実パス導入の余地）", async () => {
    const config = await requestImageUpload(
      png,
      () => true,
      undefined,
      (f) => `/home/user/pics/${f.name}`
    );
    expect(config?.uploadedImagePath).toBe("/home/user/pics/me.png");
  });
});
