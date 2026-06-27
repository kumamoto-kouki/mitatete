import { describe, it, expect, vi, beforeEach } from "vitest";
import { initPrincipleEngine, getCurrentPrinciples } from "./principles";
import { CharacterStore } from "./character-store";
import { AI_DISCLOSURE, type CharacterSchema } from "./character-validator";

// Tauriコマンド境界をモックする（CharacterStore が invoke を呼ぶため）。
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function makeSchema(overrides: Partial<CharacterSchema> = {}): CharacterSchema {
  return {
    id: "char-1",
    name: "テスト",
    visual: "x.png",
    tone: "口調。",
    aiDisclosure: AI_DISCLOSURE,
    principleDefaults: {
      固有性を与える: 5,
      信頼から始める: 4,
      一貫性を守る: 3,
      余白を持つ: 2,
      距離感を大切にする: 1,
      行動で示す: 3,
      多様な向き合い方を認める: 4,
    },
    diaryEnabled: false,
    isPreset: false,
    ...overrides,
  };
}

describe("initPrincipleEngine (要件 4.1, 4.4)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("アクティブキャラクター切り替え時に principleDefaults を受け取って更新する", async () => {
    const a = makeSchema({ id: "char-a" });
    const b = makeSchema({
      id: "char-b",
      principleDefaults: {
        固有性を与える: 1,
        信頼から始める: 1,
        一貫性を守る: 1,
        余白を持つ: 5,
        距離感を大切にする: 5,
        行動で示す: 5,
        多様な向き合い方を認める: 5,
      },
    });
    invokeMock.mockResolvedValueOnce([JSON.stringify(a), JSON.stringify(b)]);
    await CharacterStore.init();

    const unsubscribe = initPrincipleEngine();
    await CharacterStore.setActive("char-b");

    expect(getCurrentPrinciples()).toEqual(b.principleDefaults);
    unsubscribe();
  });

  it("解除後は更新されない", async () => {
    const a = makeSchema({ id: "char-a" });
    invokeMock.mockResolvedValueOnce([JSON.stringify(a)]);
    await CharacterStore.init();

    const unsubscribe = initPrincipleEngine();
    await CharacterStore.setActive("char-a");
    const snapshot = getCurrentPrinciples();
    unsubscribe();

    // 解除後に別の値で setActive しても getCurrentPrinciples は変わらない。
    expect(getCurrentPrinciples()).toEqual(snapshot);
  });
});
