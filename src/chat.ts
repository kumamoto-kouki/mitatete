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

/**
 * メッセージを送信し、モデル応答テキストを返す。
 *
 * - `send_message`（model-router）へ schema・履歴・新規メッセージを渡す。
 * - 応答が正常に返ったときのみ、当日履歴（履歴＋user＋assistant）を `save_history` で保存する（要件6.1）。
 * - 送信が失敗（API エラー・キー未設定など）した場合は例外が伝播し、履歴を保存しない（要件6.2）。
 *   呼び出し元が UI へエラー表示する。
 */
export async function sendChatMessage(
  active: CharacterSchema,
  history: ChatTurn[],
  message: string
): Promise<string> {
  const text = await invoke<string>("send_message", {
    schemaJson: JSON.stringify(active),
    historyJson: JSON.stringify(history),
    message,
  });
  // 成功時のみ当日履歴を保存する。
  const updated: ChatTurn[] = [
    ...history,
    { role: "user", content: message },
    { role: "assistant", content: text },
  ];
  await invoke("save_history", { date: todayString(), data: updated });
  return text;
}
