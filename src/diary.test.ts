import { describe, it, expect, vi, beforeEach } from "vitest";
import { AI_DISCLOSURE, type CharacterSchema } from "./character-validator";

// ─── Tauri invoke モック ───────────────────────────────────────────────────────
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// ─── character-store モック ────────────────────────────────────────────────────
const getActiveMock = vi.fn<() => CharacterSchema | null>();
vi.mock("./character-store", () => ({
  CharacterStore: {
    getActive: () => getActiveMock(),
    subscribe: vi.fn(() => () => {}),
  },
}));

// ─── テスト用キャラクタースキーマ（原則9 ON） ────────────────────────────────
const activeOn: CharacterSchema = {
  id: "c1",
  name: "ミタ",
  visual: "",
  tone: "丁寧。",
  aiDisclosure: AI_DISCLOSURE,
  principleDefaults: {
    固有性を与える: 3,
    信頼から始める: 3,
    一貫性を守る: 3,
    余白を持つ: 3,      // 3 * 0.4 = 1.2
    距離感を大切にする: 3, // 3 * 0.3 = 0.9
    行動で示す: 3,      // 3 * 0.1 = 0.3
    多様な向き合い方を認める: 3, // 3 * 0.2 = 0.6 → 合計 3.0 → short
  },
  diaryEnabled: true,
  isPreset: false,
};

// 原則9 OFF キャラクター
const activeOff: CharacterSchema = { ...activeOn, diaryEnabled: false };

// ─── generateTodaysDiary テスト ───────────────────────────────────────────────
describe("generateTodaysDiary", () => {
  // モジュールは各 describe 内で動的インポートする（モック差し替えのため）
  beforeEach(() => {
    invokeMock.mockReset();
    getActiveMock.mockReset();
  });

  it("原則9 OFF：生成・read_history・save_diary を呼ばない（要件 1.2）", async () => {
    getActiveMock.mockReturnValue(activeOff);

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    expect(result.status).toBe("disabled");
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("アクティブキャラクターが null のとき disabled を返す", async () => {
    getActiveMock.mockReturnValue(null);

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    expect(result.status).toBe("disabled");
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("当日履歴なし：生成・save_diary を呼ばず no_history を返す（要件 3.2）", async () => {
    getActiveMock.mockReturnValue(activeOn);
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "read_history") return Promise.resolve(null);
      return Promise.resolve();
    });

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    expect(result.status).toBe("no_history");
    // read_history は呼ぶが、generate_text と save_diary は呼ばない
    expect(invokeMock.mock.calls.some((c) => c[0] === "read_history")).toBe(true);
    expect(invokeMock.mock.calls.some((c) => c[0] === "generate_text")).toBe(false);
    expect(invokeMock.mock.calls.some((c) => c[0] === "save_diary")).toBe(false);
  });

  it("履歴が空配列のとき：生成・save_diary を呼ばず no_history を返す（要件 3.2）", async () => {
    getActiveMock.mockReturnValue(activeOn);
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "read_history") return Promise.resolve([]);
      return Promise.resolve();
    });

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    expect(result.status).toBe("no_history");
    expect(invokeMock.mock.calls.some((c) => c[0] === "generate_text")).toBe(false);
    expect(invokeMock.mock.calls.some((c) => c[0] === "save_diary")).toBe(false);
  });

  it("read_history が reject：no_history へ縮退し generate_text/save_diary を呼ばない（守屋レビュー W-2・I/O 失敗の fail-safe）", async () => {
    getActiveMock.mockReturnValue(activeOn);
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "read_history")
        return Promise.reject(new Error("io error: read failed"));
      return Promise.resolve();
    });

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    // 失敗は「空履歴」と同等扱い（throw を握り潰さず no_history で返す）
    expect(result.status).toBe("no_history");
    expect(invokeMock.mock.calls.some((c) => c[0] === "read_history")).toBe(true);
    expect(invokeMock.mock.calls.some((c) => c[0] === "generate_text")).toBe(false);
    expect(invokeMock.mock.calls.some((c) => c[0] === "save_diary")).toBe(false);
  });

  it("正常時：生成→表示→save_diary を内容改変なしで呼ぶ（要件 5.1, 5.2, 5.4）", async () => {
    getActiveMock.mockReturnValue(activeOn);
    const history = [
      { role: "user", content: "こんにちは" },
      { role: "assistant", content: "はい" },
    ];
    const generatedText = "今日、ミタとして対話を記録する。\n観察内容。";

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "read_history") return Promise.resolve(history);
      if (cmd === "generate_text") return Promise.resolve(generatedText);
      if (cmd === "save_diary") return Promise.resolve({ status: "LocalOnly" });
      return Promise.resolve();
    });

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      // 内容改変なしで save_diary を呼ぶ（要件 5.4）
      expect(result.content).toBe(generatedText);
    }

    // generate_text を呼ぶ
    expect(invokeMock.mock.calls.some((c) => c[0] === "generate_text")).toBe(true);

    // save_diary を今日の日付・内容で呼ぶ（要件 5.2）
    const saveCall = invokeMock.mock.calls.find((c) => c[0] === "save_diary");
    expect(saveCall).toBeTruthy();
    const saveArgs = saveCall![1] as { date: string; content: string };
    expect(saveArgs.content).toBe(generatedText);
    expect(saveArgs.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("generate_text に正しい system_prompt と history_json を渡す（要件 4）", async () => {
    getActiveMock.mockReturnValue(activeOn);
    const history = [{ role: "user", content: "テスト" }];

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "read_history") return Promise.resolve(history);
      if (cmd === "generate_text") return Promise.resolve("観察文");
      if (cmd === "save_diary") return Promise.resolve({ status: "LocalOnly" });
      return Promise.resolve();
    });

    const { generateTodaysDiary } = await import("./diary");
    await generateTodaysDiary();

    const genCall = invokeMock.mock.calls.find((c) => c[0] === "generate_text");
    expect(genCall).toBeTruthy();
    const genArgs = genCall![1] as { systemPrompt: string; historyJson: string };

    // system_prompt に固定書き出しと AI 明示が含まれること（要件 4.2, 4.4）
    expect(genArgs.systemPrompt).toContain("ミタとして対話を記録する。");
    expect(genArgs.systemPrompt).toContain("AI");

    // history_json が当日履歴の JSON であること（要件 3.1）
    const parsedHistory = JSON.parse(genArgs.historyJson);
    expect(parsedHistory).toEqual(history);
  });

  it("生成失敗時：save_diary を呼ばず error を返す（要件 6.1, 6.2）", async () => {
    getActiveMock.mockReturnValue(activeOn);
    const history = [{ role: "user", content: "こんにちは" }];

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "read_history") return Promise.resolve(history);
      if (cmd === "generate_text")
        return Promise.reject({ kind: "ApiKeyMissing", message: "claude" });
      return Promise.resolve();
    });

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    expect(result.status).toBe("error");
    // save_diary を呼ばない（要件 6.2）
    expect(invokeMock.mock.calls.some((c) => c[0] === "save_diary")).toBe(false);
  });

  it("save_diary 失敗時：生成テキストは返す（保存失敗は応答を壊さない）", async () => {
    getActiveMock.mockReturnValue(activeOn);
    const history = [{ role: "user", content: "こんにちは" }];
    const generatedText = "観察文テスト";

    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "read_history") return Promise.resolve(history);
      if (cmd === "generate_text") return Promise.resolve(generatedText);
      if (cmd === "save_diary") return Promise.reject(new Error("disk full"));
      return Promise.resolve();
    });

    const { generateTodaysDiary } = await import("./diary");
    const result = await generateTodaysDiary();

    // 生成は成功しているので ok で content は返す
    expect(result.status).toBe("ok");
    if (result.status === "ok") {
      expect(result.content).toBe(generatedText);
      expect(result.saved).toBe(false);
    }
  });
});
