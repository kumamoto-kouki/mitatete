// チャットUI のエントリ（スタブ）。
// model-router spec の実装時に、選択中キャラクター＋原則値でプロンプトを構築し、
// Rust バックエンド（Tauri コマンド）経由でモデルAPIへ送信する。

type Role = "user" | "assistant";

const form = document.querySelector<HTMLFormElement>("#composer");
const input = document.querySelector<HTMLInputElement>("#input");
const messages = document.querySelector<HTMLElement>("#messages");

form?.addEventListener("submit", (e: SubmitEvent) => {
  e.preventDefault();
  const text = input?.value.trim() ?? "";
  if (!text || !input) return;
  appendMessage("user", text);
  input.value = "";
  // TODO(model-router): Tauri コマンド経由でモデルへ送信し応答を表示する
});

function appendMessage(role: Role, text: string): void {
  if (!messages) return;
  const el = document.createElement("div");
  el.className = `msg msg--${role}`;
  el.textContent = text;
  messages.appendChild(el);
  messages.scrollTop = messages.scrollHeight;
}
