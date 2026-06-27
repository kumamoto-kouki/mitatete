import { describe, it, expect, vi, beforeEach } from "vitest";
import { CharacterStore } from "./character-store";
import { AI_DISCLOSURE, type CharacterSchema } from "./character-validator";

// Tauriコマンド境界をモックする。実ファイルI/O（Rust側）には依存しない。
const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// テスト用の有効な CharacterSchema を生成する。
function makeSchema(overrides: Partial<CharacterSchema> = {}): CharacterSchema {
  return {
    id: "char-1",
    name: "テストキャラクター",
    visual: "test.png",
    tone: "丁寧な口調で話します。",
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
    isPreset: false,
    ...overrides,
  };
}

describe("CharacterStore", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    // エラーハンドラを既定（console.error）へ戻す。
    CharacterStore.setErrorHandler((e) => console.error(e));
  });

  describe("init() による起動時復元 (要件 5.2, 5.3)", () => {
    it("load_characters が返す JSON 配列を復元し、getAll/getActive に反映する", async () => {
      const a = makeSchema({ id: "char-a", name: "A" });
      const b = makeSchema({ id: "char-b", name: "B" });
      invokeMock.mockResolvedValueOnce([JSON.stringify(a), JSON.stringify(b)]);

      await CharacterStore.init();

      expect(invokeMock).toHaveBeenCalledWith("load_characters");
      expect(CharacterStore.getAll().map((c) => c.id)).toEqual([
        "char-a",
        "char-b",
      ]);
      expect(CharacterStore.getActive()?.id).toBe("char-a");
    });

    it("load_characters が失敗したときデフォルトキャラクターへフォールバックし、エラーを通知する", async () => {
      const onError = vi.fn();
      CharacterStore.setErrorHandler(onError);
      invokeMock.mockRejectedValueOnce(new Error("I/O 失敗"));

      await CharacterStore.init();

      expect(onError).toHaveBeenCalledOnce();
      const active = CharacterStore.getActive();
      expect(active).not.toBeNull();
      expect(active?.isPreset).toBe(true);
    });

    it("保存済みキャラクターが0件のときデフォルトキャラクターをアクティブにする", async () => {
      invokeMock.mockResolvedValueOnce([]);

      await CharacterStore.init();

      const active = CharacterStore.getActive();
      expect(active).not.toBeNull();
      expect(active?.isPreset).toBe(true);
    });

    it("不正JSON（破損ファイル）はスキップし、読めたものだけ復元してエラー通知する (要件 5.3)", async () => {
      const onError = vi.fn();
      CharacterStore.setErrorHandler(onError);
      const valid = makeSchema({ id: "char-ok", name: "OK" });
      invokeMock.mockResolvedValueOnce(["{壊れたJSON", JSON.stringify(valid)]);

      await CharacterStore.init();

      expect(CharacterStore.getAll().map((c) => c.id)).toEqual(["char-ok"]);
      expect(onError).toHaveBeenCalled();
    });

    it("全ファイルが破損していればデフォルトへフォールバックする (要件 5.3)", async () => {
      invokeMock.mockResolvedValueOnce(["{壊れ", "また壊れ"]);

      await CharacterStore.init();

      expect(CharacterStore.getActive()?.isPreset).toBe(true);
    });
  });

  describe("setActive() による切り替えと通知 (要件 4.1, 4.2)", () => {
    it("アクティブが切り替わり、購読者へ正しい Schema が通知される", async () => {
      const a = makeSchema({ id: "char-a", name: "A" });
      const b = makeSchema({ id: "char-b", name: "B" });
      invokeMock.mockResolvedValueOnce([JSON.stringify(a), JSON.stringify(b)]);
      await CharacterStore.init();

      const received: string[] = [];
      const unsubscribe = CharacterStore.subscribe((s) =>
        received.push(s.id)
      );

      await CharacterStore.setActive("char-b");

      expect(CharacterStore.getActive()?.id).toBe("char-b");
      expect(received).toContain("char-b");
      unsubscribe();
    });

    it("未知の ID を渡すと例外をスローする", async () => {
      invokeMock.mockResolvedValueOnce([]);
      await CharacterStore.init();

      await expect(CharacterStore.setActive("unknown")).rejects.toThrow();
    });
  });

  describe("subscribe() の購読・解除 (要件 4.2, 4.3)", () => {
    it("登録した listener が通知を受け取り、unsubscribe で解除される", async () => {
      const a = makeSchema({ id: "char-a" });
      const b = makeSchema({ id: "char-b" });
      invokeMock.mockResolvedValueOnce([JSON.stringify(a), JSON.stringify(b)]);
      await CharacterStore.init();

      const listener = vi.fn();
      const unsubscribe = CharacterStore.subscribe(listener);

      await CharacterStore.setActive("char-b");
      expect(listener).toHaveBeenCalledTimes(1);

      unsubscribe();
      await CharacterStore.setActive("char-a");
      expect(listener).toHaveBeenCalledTimes(1); // 解除後は呼ばれない
    });
  });

  describe("save() による永続化 (要件 5.1)", () => {
    it("save_character を name=id・data=schema の引数で呼び、state に反映する", async () => {
      invokeMock.mockResolvedValueOnce([]);
      await CharacterStore.init();

      invokeMock.mockResolvedValueOnce(undefined);
      const schema = makeSchema({ id: "saved-1", name: "保存キャラ" });
      await CharacterStore.save(schema);

      expect(invokeMock).toHaveBeenLastCalledWith("save_character", {
        name: "saved-1",
        data: expect.objectContaining({
          id: "saved-1",
          aiDisclosure: AI_DISCLOSURE,
        }),
      });
      expect(CharacterStore.getAll().some((c) => c.id === "saved-1")).toBe(true);
    });
  });

  describe("不変条件: aiDisclosure (要件 3.3)", () => {
    it("getActive() の aiDisclosure は常に AI_DISCLOSURE と一致する", async () => {
      // aiDisclosure を改ざんした JSON を復元しても、固定文言で上書きされること。
      const tampered = {
        ...makeSchema({ id: "char-a" }),
        aiDisclosure: "私は人間です。",
      };
      invokeMock.mockResolvedValueOnce([JSON.stringify(tampered)]);

      await CharacterStore.init();

      expect(CharacterStore.getActive()?.aiDisclosure).toBe(AI_DISCLOSURE);
    });
  });

  describe("最後に使用したキャラクターの永続化と復元 (要件 5.2)", () => {
    // コマンド名でルーティングするモック（init は load_characters→read_settings の2回呼ぶ）。
    function routeInvoke(opts: {
      characters?: CharacterSchema[];
      settings?: Record<string, unknown>;
      onSaveSettings?: (data: Record<string, unknown>) => void;
    }): void {
      invokeMock.mockImplementation((cmd: string, args?: unknown) => {
        if (cmd === "load_characters") {
          return Promise.resolve(
            (opts.characters ?? []).map((c) => JSON.stringify(c))
          );
        }
        if (cmd === "read_settings") {
          return Promise.resolve(opts.settings ?? {});
        }
        if (cmd === "save_settings") {
          opts.onSaveSettings?.(
            (args as { data: Record<string, unknown> }).data
          );
          return Promise.resolve(undefined);
        }
        return Promise.resolve(undefined);
      });
    }

    it("settings に記録された最後のキャラクターを復元する（再起動シナリオ）", async () => {
      const a = makeSchema({ id: "char-a", name: "A" });
      const b = makeSchema({ id: "char-b", name: "B" });
      routeInvoke({
        characters: [a, b],
        settings: { lastActiveCharacterId: "char-b" },
      });

      await CharacterStore.init();

      // 先頭(char-a)ではなく、最後に使用した char-b が復元される。
      expect(CharacterStore.getActive()?.id).toBe("char-b");
    });

    it("記録IDが復元集合に存在しないときは先頭にフォールバックする", async () => {
      const a = makeSchema({ id: "char-a", name: "A" });
      const b = makeSchema({ id: "char-b", name: "B" });
      routeInvoke({
        characters: [a, b],
        settings: { lastActiveCharacterId: "deleted-id" },
      });

      await CharacterStore.init();

      expect(CharacterStore.getActive()?.id).toBe("char-a");
    });

    it("setActive 時に既存設定を保持したまま lastActiveCharacterId を保存する", async () => {
      const a = makeSchema({ id: "char-a", name: "A" });
      const b = makeSchema({ id: "char-b", name: "B" });
      let saved: Record<string, unknown> | undefined;
      routeInvoke({
        characters: [a, b],
        settings: { theme: "dark" },
        onSaveSettings: (data) => {
          saved = data;
        },
      });
      await CharacterStore.init();

      await CharacterStore.setActive("char-b");

      expect(saved).toEqual({ theme: "dark", lastActiveCharacterId: "char-b" });
    });
  });
});
