import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  loadPresets,
  selectPreset,
  switchCharacter,
  connectCrossWindow,
  CHARACTER_CHANGED_EVENT,
} from "./character-ui";
import { CharacterStore } from "./character-store";
import { AI_DISCLOSURE, type CharacterSchema } from "./character-validator";

// Tauriコマンド境界をモックする（CharacterStore.save/setActive が invoke を呼ぶため）。
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// Tauri イベント境界をモックする（別ウィンドウ放送用 emit）。
const emitMock = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => emitMock(...args),
  listen: vi.fn(),
}));

// fetch をモックする。実ネットワーク／静的アセットには依存しない。
const fetchMock = vi.fn();

function jsonResponse(body: unknown, ok = true, status = 200): Response {
  return {
    ok,
    status,
    json: async () => body,
  } as unknown as Response;
}

describe("loadPresets (要件 1.1, 1.5)", () => {
  beforeEach(() => {
    fetchMock.mockReset();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("マニフェストと各定義を読み込み、CharacterSchema 配列を返す", async () => {
    const presetA = { id: "preset-a", name: "A", isPreset: true };
    const presetB = { id: "preset-b", name: "B", isPreset: true };
    fetchMock
      .mockResolvedValueOnce(jsonResponse(["preset-a.json", "preset-b.json"]))
      .mockResolvedValueOnce(jsonResponse(presetA))
      .mockResolvedValueOnce(jsonResponse(presetB));

    const result = await loadPresets();

    expect(fetchMock).toHaveBeenNthCalledWith(1, "/presets/index.json");
    expect(fetchMock).toHaveBeenNthCalledWith(2, "/presets/preset-a.json");
    expect(result.map((p) => p.id)).toEqual(["preset-a", "preset-b"]);
  });

  it("マニフェスト取得に失敗したとき空配列を返し、エラーを通知する", async () => {
    const onError = vi.fn();
    fetchMock.mockResolvedValueOnce(jsonResponse(null, false, 404));

    const result = await loadPresets(onError);

    expect(result).toEqual([]);
    expect(onError).toHaveBeenCalledOnce();
  });

  it("個別の定義ファイルが欠損していても、読めたものだけを返す（部分縮退）", async () => {
    const onError = vi.fn();
    fetchMock
      .mockResolvedValueOnce(jsonResponse(["preset-a.json", "broken.json"]))
      .mockResolvedValueOnce(jsonResponse({ id: "preset-a", name: "A" }))
      .mockResolvedValueOnce(jsonResponse(null, false, 404));

    const result = await loadPresets(onError);

    expect(result.map((p) => p.id)).toEqual(["preset-a"]);
    expect(onError).toHaveBeenCalledOnce();
  });

  it("fetch が reject してもエラー通知して空配列を返す", async () => {
    const onError = vi.fn();
    fetchMock.mockRejectedValueOnce(new Error("network"));

    const result = await loadPresets(onError);

    expect(result).toEqual([]);
    expect(onError).toHaveBeenCalledOnce();
  });
});

describe("selectPreset (要件 1.2, 1.3)", () => {
  const presetCandidate: CharacterSchema = {
    id: "preset-a",
    name: "アシスタントA",
    visual: "presets/images/preset-a.png",
    tone: "丁寧で落ち着いた口調で話します。",
    aiDisclosure: AI_DISCLOSURE,
    principleDefaults: {
      固有性を与える: 3,
      信頼から始める: 4,
      一貫性を守る: 4,
      余白を持つ: 3,
      距離感を大切にする: 3,
      行動で示す: 3,
      多様な向き合い方を認める: 3,
    },
    diaryEnabled: false,
    isPreset: true,
  };

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("選択時に save_character を呼び、アクティブキャラクターが更新される", async () => {
    await selectPreset(presetCandidate);

    expect(invokeMock).toHaveBeenCalledWith(
      "save_character",
      expect.objectContaining({ name: "preset-a" })
    );
    expect(CharacterStore.getActive()?.id).toBe("preset-a");
  });

  it("aiDisclosure が固定文言で付与された Schema が保存・アクティブ化される (要件 1.3)", async () => {
    // aiDisclosure を改ざんした候補を渡しても、固定文言で上書きされること。
    const tampered = { ...presetCandidate, aiDisclosure: "私は人間です。" };

    await selectPreset(tampered);

    expect(CharacterStore.getActive()?.aiDisclosure).toBe(AI_DISCLOSURE);
  });

  it("name 不正な候補では validate が失敗し、エラー通知して保存しない", async () => {
    const onError = vi.fn();
    const invalid = { ...presetCandidate, name: "" };

    await selectPreset(invalid, onError);

    expect(onError).toHaveBeenCalledOnce();
    expect(invokeMock).not.toHaveBeenCalled();
  });
});

describe("switchCharacter / connectCrossWindow (要件 4.1, 4.4)", () => {
  const a: CharacterSchema = {
    id: "char-a",
    name: "A",
    visual: "a.png",
    tone: "口調A。",
    aiDisclosure: AI_DISCLOSURE,
    principleDefaults: {
      固有性を与える: 3,
      信頼から始める: 3,
      一貫性を守る: 3,
      余白を持つ: 3,
      距離感を大切にする: 3,
      行動で示す: 3,
      多様な向き合い方を認める: 3,
    },
    diaryEnabled: false,
    isPreset: true,
  };
  const b: CharacterSchema = { ...a, id: "char-b", name: "B" };

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
    emitMock.mockReset();
  });

  it("switchCharacter でアクティブが切り替わる", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(a), JSON.stringify(b)]);
    await CharacterStore.init();

    await switchCharacter("char-b");

    expect(CharacterStore.getActive()?.id).toBe("char-b");
  });

  it("connectCrossWindow 接続後、切り替えで character:changed イベントが放送される", async () => {
    invokeMock.mockResolvedValueOnce([JSON.stringify(a), JSON.stringify(b)]);
    await CharacterStore.init();

    const unsubscribe = connectCrossWindow();
    await switchCharacter("char-b");

    expect(emitMock).toHaveBeenCalledWith(
      CHARACTER_CHANGED_EVENT,
      expect.objectContaining({ id: "char-b", name: "B" })
    );
    unsubscribe();
  });
});
