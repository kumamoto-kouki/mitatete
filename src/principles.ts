// 原則エンジン（スタブ）。
// 調整可能な7原則の優先度・強度（1〜5）を7角グラフUIで編集する。
// 原則8は固定（AIであることを隠さない）、原則9は ON/OFF＋強度自動導出。

// 調整可能な7原則
export type AdjustablePrinciple =
  | "固有性を与える"
  | "信頼から始める"
  | "一貫性を守る"
  | "余白を持つ"
  | "距離感を大切にする"
  | "行動で示す"
  | "多様な向き合い方を認める";

export type PrincipleValues = Record<AdjustablePrinciple, number>;

// 原則9の強度導出式（concept.md / tech.md と一致させること）
export function calcDiaryIntensity(principles: PrincipleValues): number {
  return (
    principles["余白を持つ"] * 0.4 +
    principles["距離感を大切にする"] * 0.3 +
    principles["多様な向き合い方を認める"] * 0.2 +
    principles["行動で示す"] * 0.1
  );
}
