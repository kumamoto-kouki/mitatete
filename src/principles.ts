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

// ─── キャラクターストアとの接続（要件 4.1, 4.4） ───────────────────────────────
import { CharacterStore } from "./character-store";
import type { CharacterSchema } from "./character-validator";

// アクティブキャラクターから受け取った 7原則の現在値。未接続なら null。
let currentPrinciples: PrincipleValues | null = null;

/** 原則エンジンが保持する現在の 7原則値を返す。 */
export function getCurrentPrinciples(): PrincipleValues | null {
  return currentPrinciples;
}

/**
 * 原則エンジンをキャラクターストアに接続する。(要件 4.1, 4.4)
 *
 * アクティブキャラクターが切り替わるたびに `principleDefaults` を受け取り、
 * 7原則の現在値を更新する。原則エンジンは character ウィンドウと同じく store の購読者であり、
 * 切り替え時に即座に更新される。
 *
 * @returns 購読を解除する関数
 */
export function initPrincipleEngine(
  onUpdate?: (values: PrincipleValues) => void
): () => void {
  return CharacterStore.subscribe((schema: CharacterSchema) => {
    currentPrinciples = { ...schema.principleDefaults };
    onUpdate?.(currentPrinciples);
  });
}

// main ウィンドウ読み込み時に接続する。
if (typeof document !== "undefined") {
  initPrincipleEngine();
}
