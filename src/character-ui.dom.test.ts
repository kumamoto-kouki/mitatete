// @vitest-environment happy-dom
// E2: renderCustomList DOM テスト — カスタムカード描画・選択・編集ボタンの動作を検証する。

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  renderCustomList,
  renderPresetList,
  renderSwitcher,
} from "./character-ui";
import { CharacterStore } from "./character-store";
import { AI_DISCLOSURE, type CharacterSchema } from "./character-validator";

// Tauri コマンドをモック
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// Tauri イベントをモック
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(),
  listen: vi.fn(),
}));

// Lucide は DOM テスト環境では SVG 置換不要なのでスタブ
vi.mock("./icons", () => ({
  initIcons: vi.fn(),
}));

const principleDefaults: CharacterSchema["principleDefaults"] = {
  固有性を与える: 3,
  信頼から始める: 3,
  一貫性を守る: 3,
  余白を持つ: 3,
  距離感を大切にする: 3,
  行動で示す: 3,
  多様な向き合い方を認める: 3,
};

const customA: CharacterSchema = {
  id: "custom-a",
  name: "カスタムA",
  visual: "",
  tone: "やわらかい口調で話します。",
  aiDisclosure: AI_DISCLOSURE,
  principleDefaults,
  diaryEnabled: false,
  isPreset: false,
};

const presetX: CharacterSchema = {
  id: "preset-x",
  name: "プリセットX",
  visual: "",
  tone: "丁寧な口調。",
  aiDisclosure: AI_DISCLOSURE,
  principleDefaults,
  diaryEnabled: false,
  isPreset: true,
};

describe("renderCustomList (E2: カスタムカード描画・選択・編集)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("カスタムキャラが store にある場合、mtt-char カードが描画される", async () => {
    await CharacterStore.save(customA);
    await CharacterStore.save(presetX);

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const cards = container.querySelectorAll("[data-custom-id]");
    expect(cards.length).toBe(1);
    expect((cards[0] as HTMLElement).dataset.customId).toBe("custom-a");
  });

  it("プリセットのみの場合、コンテナは空（カード無し）", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(presetX)]);
    await CharacterStore.init();

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    expect(container.querySelectorAll("[data-custom-id]").length).toBe(0);
  });

  it("カードクリックで onSelect が呼ばれる", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const onSelect = vi.fn();
    const container = document.createElement("div");
    document.body.appendChild(container);
    renderCustomList(container, onSelect, vi.fn());

    const card = container.querySelector<HTMLButtonElement>("[data-custom-id='custom-a']")!;
    card.click();

    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "custom-a" }));
    document.body.removeChild(container);
  });

  it("編集ボタンクリックで onEdit が呼ばれ、カード選択は起きない", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const onEdit = vi.fn();
    const onSelect = vi.fn();
    const container = document.createElement("div");
    document.body.appendChild(container);
    renderCustomList(container, onSelect, onEdit);

    const editBtn = container.querySelector<HTMLButtonElement>(".character-panel__edit-btn")!;
    editBtn.click();

    expect(onEdit).toHaveBeenCalledWith(expect.objectContaining({ id: "custom-a" }));
    expect(onSelect).not.toHaveBeenCalled();
    document.body.removeChild(container);
  });

  it("削除ボタンが aria-label 付きで描画される", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn(), vi.fn());

    const deleteBtn = container.querySelector<HTMLButtonElement>(
      ".character-panel__delete-btn"
    );
    expect(deleteBtn).not.toBeNull();
    expect(deleteBtn?.getAttribute("aria-label")).toContain("削除");
  });

  it("削除ボタンクリックで onDelete が呼ばれ、カード選択（onSelect）は起きない", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const onDelete = vi.fn();
    const onSelect = vi.fn();
    const container = document.createElement("div");
    document.body.appendChild(container);
    renderCustomList(container, onSelect, vi.fn(), onDelete);

    const deleteBtn = container.querySelector<HTMLButtonElement>(
      ".character-panel__delete-btn"
    )!;
    deleteBtn.click();

    expect(onDelete).toHaveBeenCalledWith(expect.objectContaining({ id: "custom-a" }));
    expect(onSelect).not.toHaveBeenCalled();
    document.body.removeChild(container);
  });

  it("onDelete 未指定でも削除ボタンのクリックで例外が出ない", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const deleteBtn = container.querySelector<HTMLButtonElement>(
      ".character-panel__delete-btn"
    )!;
    expect(() => deleteBtn.click()).not.toThrow();
  });

  it("カスタムが複数ある場合、重複なく全件描画される", async () => {
    const customB: CharacterSchema = { ...customA, id: "custom-b", name: "カスタムB" };
    await CharacterStore.save(customA);
    await CharacterStore.save(customB);

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const cards = container.querySelectorAll("[data-custom-id]");
    const ids = [...cards].map((el) => (el as HTMLElement).dataset.customId);
    expect(ids).toContain("custom-a");
    expect(ids).toContain("custom-b");
    // 重複がないこと
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("カードに mtt-char クラスと data-custom-id が付与されている", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const card = container.querySelector("[data-custom-id='custom-a']")!;
    expect(card.classList.contains("mtt-char")).toBe(true);
    expect(card.classList.contains("character-panel__item")).toBe(true);
  });

  it("「あなたのキャラクター」セクション見出しが描画される", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const heading = container.querySelector(".character-panel__section-heading");
    expect(heading).not.toBeNull();
    expect(heading?.textContent).toContain("あなたのキャラクター");
  });
});

describe("renderSwitcher の全隠し (切り替え対象が1件以下なら隠す)", () => {
  const charB: CharacterSchema = { ...customA, id: "char-b", name: "B" };

  it("0件のときコンテナを hidden にし、何も描画しない", () => {
    const container = document.createElement("div");
    renderSwitcher(container, [], null, vi.fn());

    expect(container.hidden).toBe(true);
    expect(container.querySelector("select")).toBeNull();
  });

  it("1件のときも hidden（選択肢が1つなら切り替え不要＝全隠し）", () => {
    const container = document.createElement("div");
    renderSwitcher(container, [customA], "custom-a", vi.fn());

    expect(container.hidden).toBe(true);
    expect(container.querySelector("select")).toBeNull();
  });

  it("2件以上のとき表示し、select に件数分の option と activeId 選択を反映する", () => {
    const container = document.createElement("div");
    renderSwitcher(container, [customA, charB], "char-b", vi.fn());

    expect(container.hidden).toBe(false);
    const select = container.querySelector<HTMLSelectElement>("select")!;
    expect(select.querySelectorAll("option").length).toBe(2);
    expect(select.value).toBe("char-b");
  });

  it("2件→1件に減った再描画で隠れる（残留しない）", () => {
    const container = document.createElement("div");
    renderSwitcher(container, [customA, charB], "char-b", vi.fn());
    expect(container.hidden).toBe(false);

    renderSwitcher(container, [customA], "custom-a", vi.fn());
    expect(container.hidden).toBe(true);
    expect(container.querySelector("select")).toBeNull();
  });

  it("change で onSwitch(id) が発火する", () => {
    const container = document.createElement("div");
    const onSwitch = vi.fn();
    renderSwitcher(container, [customA, charB], "custom-a", onSwitch);

    const select = container.querySelector<HTMLSelectElement>("select")!;
    select.value = "char-b";
    select.dispatchEvent(new Event("change"));

    expect(onSwitch).toHaveBeenCalledWith("char-b");
  });
});

describe("D2: createAvatar — visual 指定ありのカード描画とフォールバック", () => {
  const visualSchema: CharacterSchema = {
    id: "vis-a",
    name: "ビジュアルA",
    visual: "data:image/png;base64,abc",
    tone: "明るい。",
    aiDisclosure: AI_DISCLOSURE,
    principleDefaults,
    diaryEnabled: false,
    isPreset: false,
  };

  const noVisualPreset: CharacterSchema = {
    id: "preset-novis",
    name: "プリセット無画像",
    visual: "",
    tone: "静か。",
    aiDisclosure: AI_DISCLOSURE,
    principleDefaults,
    diaryEnabled: false,
    isPreset: true,
  };

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("カスタム: visual 指定あり → img.mtt-avt__img が src/alt 付きで描画される", async () => {
    await CharacterStore.save(visualSchema);

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const card = container.querySelector("[data-custom-id='vis-a']")!;
    const img = card.querySelector<HTMLImageElement>("img.mtt-avt__img");
    expect(img).not.toBeNull();
    // happy-dom は src を解決するため getAttribute で元の値を検証する。
    expect(img?.getAttribute("src")).toBe("data:image/png;base64,abc");
    expect(img?.alt).toBe("ビジュアルA");
  });

  it("カスタム: visual 空 → img なし・頭文字テキストが span に入る", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(customA)]);
    await CharacterStore.init();

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const card = container.querySelector("[data-custom-id='custom-a']")!;
    expect(card.querySelector("img.mtt-avt__img")).toBeNull();
    const avt = card.querySelector(".mtt-avt");
    expect(avt?.textContent).toBe("カ");
  });

  it("プリセット: visual 指定あり → img.mtt-avt__img が描画される", () => {
    const presetWithVisual: CharacterSchema = {
      ...noVisualPreset,
      id: "preset-vis",
      visual: "/presets/vis.png",
    };
    const container = document.createElement("div");
    renderPresetList(container, [presetWithVisual], vi.fn(), null);

    const card = container.querySelector("[data-preset-id='preset-vis']")!;
    const img = card.querySelector<HTMLImageElement>("img.mtt-avt__img");
    expect(img).not.toBeNull();
    // happy-dom は相対パスを絶対 URL に解決するため getAttribute で検証する。
    expect(img?.getAttribute("src")).toBe("/presets/vis.png");
    expect(img?.alt).toBe("プリセット無画像");
  });

  it("プリセット: visual 空 → img なし・頭文字テキストが入る", () => {
    const container = document.createElement("div");
    renderPresetList(container, [noVisualPreset], vi.fn(), null);

    const card = container.querySelector("[data-preset-id='preset-novis']")!;
    expect(card.querySelector("img.mtt-avt__img")).toBeNull();
    const avt = card.querySelector(".mtt-avt");
    expect(avt?.textContent).toBe("プ");
  });

  it("画像 error 発火 → img が除去され頭文字にフォールバックする", async () => {
    await CharacterStore.save(visualSchema);

    const container = document.createElement("div");
    renderCustomList(container, vi.fn(), vi.fn());

    const card = container.querySelector("[data-custom-id='vis-a']")!;
    const img = card.querySelector<HTMLImageElement>("img.mtt-avt__img")!;
    expect(img).not.toBeNull();

    img.dispatchEvent(new Event("error"));

    expect(card.querySelector("img.mtt-avt__img")).toBeNull();
    const avt = card.querySelector(".mtt-avt");
    expect(avt?.textContent).toBe("ビ");
  });
});

describe("renderPresetList の選択ハイライト (M2: activeId から導出・残留しない)", () => {
  const presetY: CharacterSchema = { ...presetX, id: "preset-y", name: "プリセットY" };

  function selectedIds(container: HTMLElement): string[] {
    return [...container.querySelectorAll(".character-panel__item.is-selected")].map(
      (el) => (el as HTMLElement).dataset.presetId ?? ""
    );
  }

  it("activeId に一致するプリセットだけに is-selected が付く", () => {
    const container = document.createElement("div");
    renderPresetList(container, [presetX, presetY], vi.fn(), "preset-x");

    expect(selectedIds(container)).toEqual(["preset-x"]);
  });

  it("activeId=null なら誰も選択されない", () => {
    const container = document.createElement("div");
    renderPresetList(container, [presetX, presetY], vi.fn(), null);

    expect(selectedIds(container)).toEqual([]);
  });

  it("別の activeId で再描画すると前の選択が残らない（M2 回帰ガード）", () => {
    const container = document.createElement("div");
    renderPresetList(container, [presetX, presetY], vi.fn(), "preset-x");
    expect(selectedIds(container)).toEqual(["preset-x"]);

    // カスタム選択に切り替わった想定（プリセットに一致しない activeId）で再描画。
    renderPresetList(container, [presetX, presetY], vi.fn(), "custom-a");
    expect(selectedIds(container)).toEqual([]); // プリセット側のハイライトは残留しない
  });
});
