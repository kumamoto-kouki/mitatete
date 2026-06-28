// チャットUI のエントリ。
// 選択中キャラクター（character-store）＋履歴で send_message（model-router）を呼び、応答を表示する。
// 応答成功時のみ当日履歴を保存する（chat.ts が担保、要件6.1/6.2）。

import { CharacterStore } from "./character-store";
import { sendChatMessage, type ChatTurn, type ChatRole } from "./chat";

const form = document.querySelector<HTMLFormElement>("#composer");
const input = document.querySelector<HTMLInputElement>("#input");
const messages = document.querySelector<HTMLElement>("#messages");

// このセッションの対話履歴（送信時に send_message へ渡す）。
const conversation: ChatTurn[] = [];

form?.addEventListener("submit", async (e: SubmitEvent) => {
  e.preventDefault();
  const text = input?.value.trim() ?? "";
  if (!text || !input) return;

  const active = CharacterStore.getActive();
  if (!active) {
    appendMessage("assistant", "キャラクターが選択されていません。先にキャラクターを選んでください。");
    return;
  }

  appendMessage("user", text);
  input.value = "";
  const pending = appendMessage("assistant", "…"); // 応答待ち表示（要件4.4）

  try {
    const { text: reply, saved } = await sendChatMessage(active, [...conversation], text);
    // 応答は取得できた → 必ず表示し会話へ反映する（保存可否に関わらず、QA-R1）。
    pending.textContent = reply;
    conversation.push({ role: "user", content: text });
    conversation.push({ role: "assistant", content: reply });
    if (!saved) {
      // 履歴保存だけ失敗した場合は、応答は活かしつつ控えめに注記する。
      pending.title = "この応答の履歴保存に失敗しました（会話は継続できます）。";
      pending.classList.add("msg--unsaved");
    }
  } catch (error) {
    // ModelError は { kind, message }。キー未設定は設定へ誘導する（要件3.4）。エラー時は履歴を残さない（6.2）。
    const kind = (error as { kind?: string })?.kind;
    if (kind === "ApiKeyMissing") {
      pending.textContent =
        "選択中モデルの API キーが未設定です。上部の設定パネルで API キーを設定してください。";
    } else {
      pending.textContent = `送信に失敗しました: ${(error as { message?: string })?.message ?? error}`;
    }
    pending.classList.add("msg--error");
    console.error(error);
  }
});

function appendMessage(role: ChatRole, text: string): HTMLDivElement {
  const el = document.createElement("div");
  el.className = `msg msg--${role}`;
  el.textContent = text;
  messages?.appendChild(el);
  if (messages) messages.scrollTop = messages.scrollHeight;
  return el;
}
