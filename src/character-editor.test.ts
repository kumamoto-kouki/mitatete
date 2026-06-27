import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  buildCustomCharacter,
  submitCustomCharacter,
  DEFAULT_AVATAR,
} from "./character-editor";
import { CharacterStore } from "./character-store";
import { AI_DISCLOSURE } from "./character-validator";

// Tauriコマンド境界をモックする（CharacterStore.save が invoke を呼ぶため）。
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("buildCustomCharacter (要件 2.3, 2.4)", () => {
  it("ビジュアル未設定のときデフォルトアバターを適用する (要件 2.4)", () => {
    const schema = buildCustomCharacter({
      name: "カスタム",
      tone: "やわらかい口調で話します。",
    });
    expect(schema.visual).toBe(DEFAULT_AVATAR);
  });

  it("ビジュアルが空文字のときもデフォルトアバターを適用する", () => {
    const schema = buildCustomCharacter({
      name: "カスタム",
      tone: "やわらかい口調で話します。",
      visual: "   ",
    });
    expect(schema.visual).toBe(DEFAULT_AVATAR);
  });

  it("ビジュアル指定があればそれを使う", () => {
    const schema = buildCustomCharacter({
      name: "カスタム",
      tone: "やわらかい口調で話します。",
      visual: "data:image/png;base64,AAA",
    });
    expect(schema.visual).toBe("data:image/png;base64,AAA");
  });

  it("aiDisclosure は固定文言で付与され、isPreset は false (要件 2.3)", () => {
    const schema = buildCustomCharacter({
      name: "カスタム",
      tone: "やわらかい口調で話します。",
    });
    expect(schema.aiDisclosure).toBe(AI_DISCLOSURE);
    expect(schema.isPreset).toBe(false);
  });

  it("name が空のとき例外をスローする (要件 2.1)", () => {
    expect(() =>
      buildCustomCharacter({ name: "  ", tone: "口調あり。" })
    ).toThrow();
  });

  it("tone が空のとき例外をスローする (要件 2.1)", () => {
    expect(() => buildCustomCharacter({ name: "名前", tone: "" })).toThrow();
  });
});

describe("submitCustomCharacter (要件 2.5)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("save_character を呼んでカスタムキャラクターを永続化する", async () => {
    const schema = await submitCustomCharacter({
      name: "カスタム花子",
      tone: "ていねいな口調。",
    });

    expect(invokeMock).toHaveBeenCalledWith(
      "save_character",
      expect.objectContaining({
        name: schema.id,
        data: expect.objectContaining({
          name: "カスタム花子",
          isPreset: false,
          aiDisclosure: AI_DISCLOSURE,
        }),
      })
    );
    expect(CharacterStore.getAll().some((c) => c.id === schema.id)).toBe(true);
  });

  it("作成のみ行い、自動でアクティブ化はしない（切り替えは6.1の責務）", async () => {
    await submitCustomCharacter({ name: "保存のみ", tone: "口調。" });

    // setActive 由来の invoke は無く、save_character のみが呼ばれる。
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith(
      "save_character",
      expect.anything()
    );
  });
});
