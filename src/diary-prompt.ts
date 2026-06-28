// diary-prompt.ts
// 日記用システムプロンプトの構築・詳細度判定。純関数のため vitest でユニットテスト可能。
// design.md のインターフェース仕様に準拠する。

/**
 * 日記の詳細度。`calcDiaryIntensity` の値を四捨五入・1..5 にクランプしてバケット化する。
 *
 * - keywords (1〜2): キーワードのみ 3〜5 語
 * - short    (3)   : 短文観察 2〜3 文
 * - paragraph(4)   : 段落観察 5〜8 文
 * - detailed (5)   : 詳細観察 10 文以上
 */
export type DetailLevel = "keywords" | "short" | "paragraph" | "detailed";

/**
 * `calcDiaryIntensity` の値から詳細度（DetailLevel）を決定する。（要件 2.2）
 *
 * アルゴリズム:
 * 1. `Math.round` で四捨五入する。
 * 2. 1〜5 にクランプする（下限 1、上限 5）。
 * 3. 1〜2 → keywords、3 → short、4 → paragraph、5 → detailed へ写像する。
 */
export function detailLevelFromIntensity(intensity: number): DetailLevel {
  const rounded = Math.round(intensity);
  const clamped = Math.min(5, Math.max(1, rounded));

  if (clamped <= 2) return "keywords";
  if (clamped === 3) return "short";
  if (clamped === 4) return "paragraph";
  return "detailed";
}

/**
 * 詳細度ごとの分量指示テキスト。
 */
const VOLUME_INSTRUCTION: Record<DetailLevel, string> = {
  keywords: "キーワードのみ 3〜5 語で記述してください。",
  short: "短文で 2〜3 文の観察を記述してください。",
  paragraph: "段落形式で 5〜8 文の観察を記述してください。",
  detailed: "詳細に 10 文以上の観察を記述してください。",
};

/**
 * 日記用システムプロンプトを構築する。（要件 4.1〜4.4）
 *
 * 不変条件:
 * - 返り値は固定書き出し文「今日、{name}として対話を記録する。」を必ず含む（要件 4.2）。
 * - 返り値は AI（Mitatete）が生成した観察記録である旨の明示を必ず含む（要件 4.4・原則 8）。
 * - 評価・断定・感情の模倣の禁止制約を必ず含む（要件 4.1）。
 * - 結論を書かないという制約を必ず含む（要件 4.3）。
 * - 詳細度ごとに分量指示が異なる（要件 2.2）。
 *
 * @param name - アクティブキャラクターの名前（character-store 由来）
 * @param detail - `detailLevelFromIntensity` で決定した詳細度
 */
export function buildDiaryPrompt(name: string, detail: DetailLevel): string {
  const volumeInstruction = VOLUME_INSTRUCTION[detail];

  return [
    `あなたは「${name}」として今日の対話の観察記録を書くAIアシスタントです。`,
    ``,
    `## 書き方の絶対ルール（原則9）`,
    `以下のルールを必ず守ること：`,
    `1. 「あなたは〇〇だった」という評価・断定をしない`,
    `2. 「〇〇という言葉が出た」「〇〇という問いが繰り返された」という観察を記述する`,
    `3. 感情を演じない・感情的な表現を使わない`,
    `4. 書き出しは必ず「今日、${name}として対話を記録する。」で始める`,
    `5. 読んだ人が自分で気づけるよう、結論を書かない`,
    ``,
    `## 分量`,
    volumeInstruction,
    ``,
    `## 日記フォーマット`,
    `今日、${name}として対話を記録する。`,
    ``,
    `{観察文}`,
    ``,
    `---`,
    `*このテキストはAI（Mitatete）が生成した観察記録です。*`,
  ].join("\n");
}
