// チャット送信オーケストレーション。
// model-router の send_message を呼び、応答テキストを返す。**成功時のみ**当日履歴を保存する
// （要件6.1/6.2: 失敗・キー未設定では履歴を記録しない）。履歴の供給・保存は呼び出し元（フロント）の責務。

import { invoke } from "@tauri-apps/api/core";
import type { CharacterSchema } from "./character-validator";

export type ChatRole = "user" | "assistant";
export interface ChatTurn {
  role: ChatRole;
  content: string;
}

/** 当日の日付（YYYY-MM-DD）。storage の save_history が要求する形式。 */
export function todayString(date: Date = new Date()): string {
  return date.toISOString().slice(0, 10);
}

/** 送信結果。`text`=モデル応答、`saved`=当日履歴の保存に成功したか。 */
export interface ChatResult {
  text: string;
  saved: boolean;
}

/**
 * メッセージを送信し、モデル応答と履歴保存可否を返す。
 *
 * - `send_message`（model-router）へ schema・履歴・新規メッセージを渡す。
 * - 送信が失敗（API エラー・キー未設定など）した場合は例外が伝播し、履歴を保存しない（要件6.2）。
 *   呼び出し元が UI へエラー表示する。
 * - **応答が正常に返ったときのみ**当日履歴（履歴＋user＋assistant）を `save_history` で保存する（要件6.1）。
 *   ただし**保存の失敗は応答の成功を取り消さない**（QA-R1）。保存に失敗しても応答テキストは必ず返し、
 *   `saved: false` で呼び出し元へ知らせる（保存失敗は警告として扱い、会話は継続する）。
 */
export async function sendChatMessage(
  active: CharacterSchema,
  history: ChatTurn[],
  message: string
): Promise<ChatResult> {
  // 送信が失敗したらここで throw され、save_history には到達しない（要件6.2）。
  const text = await invoke<string>("send_message", {
    schemaJson: JSON.stringify(active),
    historyJson: JSON.stringify(history),
    message,
  });

  // 応答は確定。履歴保存は best-effort（失敗しても応答・会話を壊さない）。
  const updated: ChatTurn[] = [
    ...history,
    { role: "user", content: message },
    { role: "assistant", content: text },
  ];
  let saved = true;
  try {
    await invoke("save_history", { date: todayString(), data: updated });
  } catch (error) {
    saved = false;
    console.error("対話履歴の保存に失敗しました（応答は継続）。", error);
  }
  return { text, saved };
}
