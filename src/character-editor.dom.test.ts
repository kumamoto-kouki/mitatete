// @vitest-environment happy-dom
// 観察日記トグル（D1）の DOM 描画・既定値・保存反映テスト。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { initCharacterEditor } from "./character-editor";

// Tauri コマンドをモック（submitCustomCharacter → CharacterStore.save → invoke）
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// character-visual-editor の Tauri 依存をスタブ（happy-dom 環境で安全に動かすため）
vi.mock("./character-visual-editor", () => ({
  initVisualEditor: (container: HTMLElement) => {
    const stub = document.createElement("div");
    stub.className = "visual-editor";
    container.appendChild(stub);
    return () => ({ mode: "template" as const, templateParams: undefined });
  },
  requestImageUpload: vi.fn(),
  buildVisualSvg: vi.fn(() => "<svg></svg>"),
  svgToDataUri: vi.fn((s: string) => `data:image/svg+xml,${encodeURIComponent(s)}`),
  DEFAULT_TEMPLATE_PARAMS: {},
}));

describe("initCharacterEditor — 観察日記トグル（D1 / 原則9）", () => {
  let root: HTMLDivElement;

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);

    // #character-editor を DOM に追加
    root = document.createElement("div");
    root.id = "character-editor";
    document.body.replaceChildren(root);

    initCharacterEditor();
  });

  it("チェックボックスが DOM に描画される", () => {
    const checkbox = document.querySelector<HTMLInputElement>(
      "#editor-diary-enabled"
    );
    expect(checkbox).not.toBeNull();
    expect(checkbox?.type).toBe("checkbox");
  });

  it("チェックボックスの既定値は OFF（false）", () => {
    const checkbox = document.querySelector<HTMLInputElement>(
      "#editor-diary-enabled"
    );
    expect(checkbox?.checked).toBe(false);
  });

  it("ラベル・ヒント文言が描画される", () => {
    const label = document.querySelector<HTMLLabelElement>(
      'label[for="editor-diary-enabled"]'
    );
    expect(label).not.toBeNull();
    expect(label?.textContent).toContain("観察日記を有効にする");

    const hint = document.querySelector<HTMLParagraphElement>(
      ".editor__diary-hint"
    );
    expect(hint).not.toBeNull();
    expect(hint?.textContent).toContain("会話の観察記録");
  });

  it("チェックボックスを ON にして保存すると diaryEnabled:true がスキーマに反映される", async () => {
    const checkbox = document.querySelector<HTMLInputElement>(
      "#editor-diary-enabled"
    )!;
    const form = document.querySelector<HTMLFormElement>("form")!;
    const nameInput = document.querySelector<HTMLInputElement>(".editor__input")!;
    const toneInput = document.querySelector<HTMLTextAreaElement>("textarea.editor__input")!;

    // 入力を埋める
    nameInput.value = "テストキャラ";
    toneInput.value = "やわらかい口調。";

    // 日記トグルを ON
    checkbox.checked = true;

    // submit イベントを発火
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    // invoke が非同期で呼ばれるのを待つ
    await new Promise((r) => setTimeout(r, 0));

    expect(invokeMock).toHaveBeenCalledWith(
      "save_character",
      expect.objectContaining({
        data: expect.objectContaining({ diaryEnabled: true }),
      })
    );
  });

  it("チェックボックスが OFF のまま保存すると diaryEnabled:false がスキーマに反映される", async () => {
    const form = document.querySelector<HTMLFormElement>("form")!;
    const nameInput = document.querySelector<HTMLInputElement>(".editor__input")!;
    const toneInput = document.querySelector<HTMLTextAreaElement>("textarea.editor__input")!;

    nameInput.value = "テストキャラ2";
    toneInput.value = "ていねいな口調。";
    // checkbox はデフォルト false のまま

    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await new Promise((r) => setTimeout(r, 0));

    expect(invokeMock).toHaveBeenCalledWith(
      "save_character",
      expect.objectContaining({
        data: expect.objectContaining({ diaryEnabled: false }),
      })
    );
  });

  it("保存成功後にチェックボックスが OFF にリセットされる", async () => {
    const checkbox = document.querySelector<HTMLInputElement>(
      "#editor-diary-enabled"
    )!;
    const form = document.querySelector<HTMLFormElement>("form")!;
    const nameInput = document.querySelector<HTMLInputElement>(".editor__input")!;
    const toneInput = document.querySelector<HTMLTextAreaElement>("textarea.editor__input")!;

    nameInput.value = "リセットテスト";
    toneInput.value = "口調。";
    checkbox.checked = true;

    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await new Promise((r) => setTimeout(r, 0));

    expect(checkbox.checked).toBe(false);
  });
});
