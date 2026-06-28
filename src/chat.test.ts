import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendChatMessage, todayString, type ChatTurn } from "./chat";
import { AI_DISCLOSURE, type CharacterSchema } from "./character-validator";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const active: CharacterSchema = {
  id: "c1",
  name: "ミタ",
  visual: "x.png",
  tone: "丁寧。",
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
  isPreset: false,
};

describe("sendChatMessage (要件 4.1, 4.2, 6.1, 6.2)", () => {
  beforeEach(() => invokeMock.mockReset());

  it("応答成功時に応答テキストを返し、当日履歴（user+assistant）を保存する", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "send_message") return Promise.resolve("こんにちは");
      if (cmd === "save_history") return Promise.resolve(undefined);
      return Promise.resolve();
    });
    const history: ChatTurn[] = [{ role: "assistant", content: "前ターン" }];

    const result = await sendChatMessage(active, history, "やあ");

    expect(result).toEqual({ text: "こんにちは", saved: true });
    // send_message に schema/history/message を渡す。
    expect(invokeMock).toHaveBeenCalledWith(
      "send_message",
      expect.objectContaining({ message: "やあ" })
    );
    // 成功時のみ save_history を呼び、user+assistant が追記される。
    const saveCall = invokeMock.mock.calls.find((c) => c[0] === "save_history");
    expect(saveCall).toBeTruthy();
    const data = (saveCall![1] as { data: ChatTurn[] }).data;
    expect(data).toHaveLength(3);
    expect(data[1]).toEqual({ role: "user", content: "やあ" });
    expect(data[2]).toEqual({ role: "assistant", content: "こんにちは" });
  });

  it("send成功×save失敗：応答は返し例外で握り潰さない（QA-R1）", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "send_message") return Promise.resolve("こんにちは");
      if (cmd === "save_history")
        return Promise.reject(new Error("disk full"));
      return Promise.resolve();
    });

    // 例外を投げず、応答テキストは返り、saved=false で保存失敗を知らせる。
    const result = await sendChatMessage(active, [], "やあ");
    expect(result.text).toBe("こんにちは");
    expect(result.saved).toBe(false);
  });

  it("送信失敗時は例外を伝播し、履歴を保存しない（要件6.2）", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "send_message")
        return Promise.reject({ kind: "ApiKeyMissing", message: "claude" });
      return Promise.resolve();
    });

    await expect(sendChatMessage(active, [], "やあ")).rejects.toBeTruthy();
    expect(invokeMock.mock.calls.some((c) => c[0] === "save_history")).toBe(
      false
    );
  });

  it("todayString は YYYY-MM-DD を返す", () => {
    expect(todayString(new Date("2026-06-27T12:34:56Z"))).toBe("2026-06-27");
  });
});
