// @vitest-environment happy-dom
// 観察日記トグル（D1）の DOM 描画・既定値・保存反映テスト。
// E2: 編集モード（populateEditor）テストも含む。

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

// ─── E2: 編集モード（populateEditor）テスト ──────────────────────────────────

describe("initCharacterEditor — 編集モード（E2）", () => {
  let root: HTMLDivElement;

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);

    root = document.createElement("div");
    root.id = "character-editor";
    document.body.replaceChildren(root);

    initCharacterEditor();
  });

  it("populateEditor が DOM に公開されている", () => {
    expect(typeof (root as unknown as Record<string, unknown>)["populateEditor"]).toBe("function");
  });

  it("populateEditor を呼ぶとフォームに schema の値が流し込まれる", () => {
    const schema = {
      id: "test-id-123",
      name: "テスト花子",
      tone: "やわらかい口調。",
      visual: "",
      aiDisclosure: "私はAIアシスタントです。人間ではありません。",
      isPreset: false,
      diaryEnabled: true,
      principleDefaults: {
        固有性を与える: 3,
        信頼から始める: 3,
        一貫性を守る: 3,
        余白を持つ: 3,
        距離感を大切にする: 3,
        行動で示す: 3,
        多様な向き合い方を認める: 3,
      },
    };

    const populate = (root as unknown as Record<string, unknown>)["populateEditor"] as (s: unknown) => void;
    populate(schema);

    const nameInput = root.querySelector<HTMLInputElement>(".editor__input")!;
    const toneInput = root.querySelector<HTMLTextAreaElement>("textarea.editor__input")!;
    const diaryCheckbox = root.querySelector<HTMLInputElement>("#editor-diary-enabled")!;

    expect(nameInput.value).toBe("テスト花子");
    expect(toneInput.value).toBe("やわらかい口調。");
    expect(diaryCheckbox.checked).toBe(true);
  });

  it("populateEditor 後に編集モードラベルが表示される", () => {
    const schema = {
      id: "edit-mode-id",
      name: "編集太郎",
      tone: "口調。",
      visual: "",
      aiDisclosure: "私はAIアシスタントです。人間ではありません。",
      isPreset: false,
      diaryEnabled: false,
      principleDefaults: {
        固有性を与える: 3,
        信頼から始める: 3,
        一貫性を守る: 3,
        余白を持つ: 3,
        距離感を大切にする: 3,
        行動で示す: 3,
        多様な向き合い方を認める: 3,
      },
    };

    const populate = (root as unknown as Record<string, unknown>)["populateEditor"] as (s: unknown) => void;
    populate(schema);

    const modeLabel = root.querySelector<HTMLElement>(".editor__mode-label")!;
    expect(modeLabel.hidden).toBe(false);
    expect(modeLabel.textContent).toContain("編集モード");
    expect(modeLabel.textContent).toContain("編集太郎");
  });

  it("編集モードで保存すると元の id（editingId）で save_character が呼ばれる", async () => {
    const schema = {
      id: "original-id-abc",
      name: "元のキャラ",
      tone: "丁寧な口調。",
      visual: "",
      aiDisclosure: "私はAIアシスタントです。人間ではありません。",
      isPreset: false,
      diaryEnabled: false,
      principleDefaults: {
        固有性を与える: 3,
        信頼から始める: 3,
        一貫性を守る: 3,
        余白を持つ: 3,
        距離感を大切にする: 3,
        行動で示す: 3,
        多様な向き合い方を認める: 3,
      },
    };

    const populate = (root as unknown as Record<string, unknown>)["populateEditor"] as (s: unknown) => void;
    populate(schema);

    // 名前を変更して保存
    const nameInput = root.querySelector<HTMLInputElement>(".editor__input")!;
    nameInput.value = "更新後キャラ";

    const form = root.querySelector<HTMLFormElement>("form")!;
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await new Promise((r) => setTimeout(r, 0));

    // 元 id（original-id-abc）で save_character が呼ばれることを確認
    expect(invokeMock).toHaveBeenCalledWith(
      "save_character",
      expect.objectContaining({
        name: "original-id-abc",
        data: expect.objectContaining({
          id: "original-id-abc",
          name: "更新後キャラ",
        }),
      })
    );
  });

  it("編集モードで保存後に新規モードへリセットされる（キャンセルと同じ動作）", async () => {
    const schema = {
      id: "reset-test-id",
      name: "リセットテスト",
      tone: "口調。",
      visual: "",
      aiDisclosure: "私はAIアシスタントです。人間ではありません。",
      isPreset: false,
      diaryEnabled: true,
      principleDefaults: {
        固有性を与える: 3,
        信頼から始める: 3,
        一貫性を守る: 3,
        余白を持つ: 3,
        距離感を大切にする: 3,
        行動で示す: 3,
        多様な向き合い方を認める: 3,
      },
    };

    const populate = (root as unknown as Record<string, unknown>)["populateEditor"] as (s: unknown) => void;
    populate(schema);

    const nameInput = root.querySelector<HTMLInputElement>(".editor__input")!;
    nameInput.value = "保存用の名前";
    const toneInput = root.querySelector<HTMLTextAreaElement>("textarea.editor__input")!;
    toneInput.value = "口調。";

    const form = root.querySelector<HTMLFormElement>("form")!;
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await new Promise((r) => setTimeout(r, 0));

    // 保存後は編集モードラベルが非表示になる
    const modeLabel = root.querySelector<HTMLElement>(".editor__mode-label")!;
    expect(modeLabel.hidden).toBe(true);

    // キャンセルボタンも非表示
    const cancelButton = root.querySelector<HTMLButtonElement>(".editor__cancel")!;
    expect(cancelButton.hidden).toBe(true);
  });

  it("キャンセルボタンで新規モードへ戻る", () => {
    const schema = {
      id: "cancel-test-id",
      name: "キャンセルテスト",
      tone: "口調。",
      visual: "",
      aiDisclosure: "私はAIアシスタントです。人間ではありません。",
      isPreset: false,
      diaryEnabled: false,
      principleDefaults: {
        固有性を与える: 3,
        信頼から始める: 3,
        一貫性を守る: 3,
        余白を持つ: 3,
        距離感を大切にする: 3,
        行動で示す: 3,
        多様な向き合い方を認める: 3,
      },
    };

    const populate = (root as unknown as Record<string, unknown>)["populateEditor"] as (s: unknown) => void;
    populate(schema);

    const cancelButton = root.querySelector<HTMLButtonElement>(".editor__cancel")!;
    expect(cancelButton.hidden).toBe(false);

    cancelButton.click();

    const modeLabel = root.querySelector<HTMLElement>(".editor__mode-label")!;
    expect(modeLabel.hidden).toBe(true);
    expect(cancelButton.hidden).toBe(true);
  });
});
