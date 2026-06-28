// @vitest-environment happy-dom
// E2: renderCustomList DOM テスト — カスタムカード描画・選択・編集ボタンの動作を検証する。

import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderCustomList, renderPresetList } from "./character-ui";
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
