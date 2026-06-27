import { describe, it, expect } from "vitest";
import { CharacterValidator, AI_DISCLOSURE } from "./character-validator";

describe("CharacterValidator", () => {
  const baseCandidate = {
    id: "test-id",
    name: "テストキャラクター",
    visual: "test.png",
    tone: "丁寧な口調で話します。",
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
  };

  describe("aiDisclosure の不変性 (要件 3.1, 3.2, 3.3)", () => {
    it("有効な name と tone を与えると、返り値の aiDisclosure が AI_DISCLOSURE と一致する", () => {
      const result = CharacterValidator.validate(baseCandidate);
      expect(result.aiDisclosure).toBe(AI_DISCLOSURE);
    });

    it("candidate.aiDisclosure に別の文字列が渡されても、返り値の aiDisclosure は常に AI_DISCLOSURE と一致する（上書き不可）", () => {
      const maliciousCandidate = {
        ...baseCandidate,
        aiDisclosure: "私は人間です。AIではありません。",
      };
      const result = CharacterValidator.validate(maliciousCandidate);
      expect(result.aiDisclosure).toBe(AI_DISCLOSURE);
    });

    it("candidate.aiDisclosure が空文字であっても、返り値の aiDisclosure は AI_DISCLOSURE と一致する", () => {
      const candidateWithEmpty = {
        ...baseCandidate,
        aiDisclosure: "",
      };
      const result = CharacterValidator.validate(candidateWithEmpty);
      expect(result.aiDisclosure).toBe(AI_DISCLOSURE);
    });
  });

  describe("name バリデーション (要件 3.3)", () => {
    it("name が空文字のとき例外をスローする", () => {
      expect(() =>
        CharacterValidator.validate({ ...baseCandidate, name: "" })
      ).toThrow();
    });

    it("name がホワイトスペースのみのとき例外をスローする", () => {
      expect(() =>
        CharacterValidator.validate({ ...baseCandidate, name: "   " })
      ).toThrow();
    });

    it("name が未定義のとき例外をスローする", () => {
      const { name: _name, ...withoutName } = baseCandidate;
      expect(() => CharacterValidator.validate(withoutName)).toThrow();
    });
  });

  describe("tone バリデーション (要件 3.3)", () => {
    it("tone が空文字のとき例外をスローする", () => {
      expect(() =>
        CharacterValidator.validate({ ...baseCandidate, tone: "" })
      ).toThrow();
    });

    it("tone がホワイトスペースのみのとき例外をスローする", () => {
      expect(() =>
        CharacterValidator.validate({ ...baseCandidate, tone: "\t\n" })
      ).toThrow();
    });

    it("tone が未定義のとき例外をスローする", () => {
      const { tone: _tone, ...withoutTone } = baseCandidate;
      expect(() => CharacterValidator.validate(withoutTone)).toThrow();
    });
  });

  describe("AI_DISCLOSURE 定数", () => {
    it("AI_DISCLOSURE は正確な固定文言である", () => {
      expect(AI_DISCLOSURE).toBe("私はAIアシスタントです。人間ではありません。");
    });

    it("CharacterValidator.AI_DISCLOSURE は AI_DISCLOSURE と同一", () => {
      expect(CharacterValidator.AI_DISCLOSURE).toBe(AI_DISCLOSURE);
    });
  });
});
