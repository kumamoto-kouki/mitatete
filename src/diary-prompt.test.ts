import { describe, it, expect } from "vitest";
import {
  detailLevelFromIntensity,
  buildDiaryPrompt,
  type DetailLevel,
} from "./diary-prompt";

// ─── detailLevelFromIntensity テスト ──────────────────────────────────────────

describe("detailLevelFromIntensity (要件 2.2)", () => {
  it("強度 1 → keywords", () => {
    expect(detailLevelFromIntensity(1)).toBe("keywords");
  });

  it("強度 2 → keywords", () => {
    expect(detailLevelFromIntensity(2)).toBe("keywords");
  });

  it("強度 3 → short", () => {
    expect(detailLevelFromIntensity(3)).toBe("short");
  });

  it("強度 4 → paragraph", () => {
    expect(detailLevelFromIntensity(4)).toBe("paragraph");
  });

  it("強度 5 → detailed", () => {
    expect(detailLevelFromIntensity(5)).toBe("detailed");
  });

  // 四捨五入と境界値のテスト
  it("強度 1.4 → 四捨五入して 1 → keywords", () => {
    expect(detailLevelFromIntensity(1.4)).toBe("keywords");
  });

  it("強度 1.5 → 四捨五入して 2 → keywords", () => {
    expect(detailLevelFromIntensity(1.5)).toBe("keywords");
  });

  it("強度 2.4 → 四捨五入して 2 → keywords", () => {
    expect(detailLevelFromIntensity(2.4)).toBe("keywords");
  });

  it("強度 2.5 → 四捨五入して 3 → short", () => {
    expect(detailLevelFromIntensity(2.5)).toBe("short");
  });

  it("強度 3.5 → 四捨五入して 4 → paragraph", () => {
    expect(detailLevelFromIntensity(3.5)).toBe("paragraph");
  });

  it("強度 4.5 → 四捨五入して 5 → detailed", () => {
    expect(detailLevelFromIntensity(4.5)).toBe("detailed");
  });

  // クランプのテスト
  it("強度 0（下限外）→ クランプして 1 → keywords", () => {
    expect(detailLevelFromIntensity(0)).toBe("keywords");
  });

  it("強度 -1（負値）→ クランプして 1 → keywords", () => {
    expect(detailLevelFromIntensity(-1)).toBe("keywords");
  });

  it("強度 6（上限外）→ クランプして 5 → detailed", () => {
    expect(detailLevelFromIntensity(6)).toBe("detailed");
  });
});

// ─── buildDiaryPrompt テスト ───────────────────────────────────────────────────

describe("buildDiaryPrompt (要件 4.1〜4.4)", () => {
  const levels: DetailLevel[] = ["keywords", "short", "paragraph", "detailed"];

  it.each(levels)(
    "詳細度 %s: 固定書き出し「今日、{name}として対話を記録する。」を含む（要件 4.2）",
    (level) => {
      const prompt = buildDiaryPrompt("テストキャラ", level);
      expect(prompt).toContain(
        "今日、テストキャラとして対話を記録する。"
      );
    }
  );

  it.each(levels)(
    "詳細度 %s: AI（Mitatete）が生成した観察記録であることの明示を含む（要件 4.4）",
    (level) => {
      const prompt = buildDiaryPrompt("テストキャラ", level);
      expect(prompt).toContain("AI");
      // Mitatete か AI 明示のいずれか
      expect(prompt.toLowerCase()).toMatch(/mitatete|ai.*生成|観察記録/i);
    }
  );

  it.each(levels)(
    "詳細度 %s: 評価・断定の禁止制約を含む（要件 4.1）",
    (level) => {
      const prompt = buildDiaryPrompt("テストキャラ", level);
      // 観察のみ・評価・断定禁止の制約が含まれること
      expect(prompt).toMatch(/評価|断定|観察/);
    }
  );

  it.each(levels)(
    "詳細度 %s: 結論を書かない制約を含む（要件 4.3）",
    (level) => {
      const prompt = buildDiaryPrompt("テストキャラ", level);
      expect(prompt).toMatch(/結論/);
    }
  );

  it("詳細度 keywords: 3〜5 語の分量指示を含む（要件 2.2）", () => {
    const prompt = buildDiaryPrompt("テストキャラ", "keywords");
    expect(prompt).toMatch(/3.{0,5}5|キーワード/);
  });

  it("詳細度 short: 2〜3 文の分量指示を含む（要件 2.2）", () => {
    const prompt = buildDiaryPrompt("テストキャラ", "short");
    expect(prompt).toMatch(/2.{0,5}3|短文/);
  });

  it("詳細度 paragraph: 5〜8 文の分量指示を含む（要件 2.2）", () => {
    const prompt = buildDiaryPrompt("テストキャラ", "paragraph");
    expect(prompt).toMatch(/5.{0,5}8|段落/);
  });

  it("詳細度 detailed: 10 文以上の分量指示を含む（要件 2.2）", () => {
    const prompt = buildDiaryPrompt("テストキャラ", "detailed");
    expect(prompt).toMatch(/10|詳細/);
  });

  it("詳細度ごとに分量指示が変わる（要件 2.2）", () => {
    const kw = buildDiaryPrompt("X", "keywords");
    const sh = buildDiaryPrompt("X", "short");
    const pa = buildDiaryPrompt("X", "paragraph");
    const de = buildDiaryPrompt("X", "detailed");
    // すべて異なること
    const set = new Set([kw, sh, pa, de]);
    expect(set.size).toBe(4);
  });

  it("name がプロンプトに反映される", () => {
    const prompt = buildDiaryPrompt("オリジナルキャラクター", "short");
    expect(prompt).toContain("オリジナルキャラクター");
  });
});
