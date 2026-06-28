// チャットUI のエントリ。
// 選択中キャラクター（character-store）＋履歴で send_message（model-router）を呼び、応答を表示する。
// 応答成功時のみ当日履歴を保存する（chat.ts が担保、要件6.1/6.2）。

import { CharacterStore } from "./character-store";
import { sendChatMessage, type ChatTurn, type ChatRole } from "./chat";
import { initIcons } from "./icons";
import { emit } from "@tauri-apps/api/event";
import { loadTheme, applyTheme, type Theme } from "./theme";

const form = document.querySelector<HTMLFormElement>("#composer");
const input = document.querySelector<HTMLTextAreaElement>("#input");
const messages = document.querySelector<HTMLElement>("#messages");
const sendBtn = document.querySelector<HTMLButtonElement>("#send-btn");
const sendIcon = document.querySelector<HTMLSpanElement>("#send-icon");
const composerInputWrap = document.querySelector<HTMLDivElement>("#composer-input-wrap");
const disclosureIcon = document.querySelector<HTMLSpanElement>("#disclosure-icon");

// ─── テーマ切替（ライト/ダーク） ────────────────────────────────────────────────

// 起動時に確定したテーマを適用（OS のダーク追従に流されないよう明示）
applyTheme(loadTheme());

// トグルクリックでライト/ダークを切り替え、character 窓へもブロードキャストする
document.querySelector<HTMLButtonElement>("#theme-toggle")?.addEventListener("click", () => {
  const current = document.documentElement.getAttribute("data-theme") as Theme;
  const next: Theme = current === "dark" ? "light" : "dark";
  applyTheme(next);
  void emit("theme:changed", next);
});

// このセッションの対話履歴（送信時に send_message へ渡す）。
const conversation: ChatTurn[] = [];

// ─── アイコン初期化（静的 HTML 部分） ───────────────────────────────────────
function mountStaticIcons(): void {
  // AI 開示バナーのアイコン
  if (disclosureIcon) {
    disclosureIcon.innerHTML = '<i data-lucide="info"></i>';
  }
  // 送信ボタンのアイコン
  if (sendIcon) {
    sendIcon.innerHTML = '<i data-lucide="arrow-up"></i>';
  }
  initIcons();
}

mountStaticIcons();

// ─── 空状態の表示 ─────────────────────────────────────────────────────────────
function showEmptyState(): void {
  if (!messages) return;
  messages.innerHTML = `
    <div class="chat__empty">
      <div class="chat__empty-icon"><i data-lucide="sparkles"></i></div>
      <p class="chat__empty-title">まだ会話がありません</p>
      <p class="chat__empty-desc">キャラクターを選んで、話しかけてみましょう。</p>
    </div>
  `;
  initIcons();
}

// 起動時に空状態を表示する
showEmptyState();

// ─── 送信ボタン ローディング状態 ──────────────────────────────────────────────
function setLoading(loading: boolean): void {
  if (!sendBtn || !sendIcon || !input || !composerInputWrap) return;
  sendBtn.disabled = loading;
  if (input instanceof HTMLTextAreaElement) input.disabled = loading;
  if (loading) {
    composerInputWrap.classList.add("is-disabled");
    sendIcon.innerHTML = '<span class="mtt-send__spinner"></span>';
  } else {
    composerInputWrap.classList.remove("is-disabled");
    sendIcon.innerHTML = '<i data-lucide="arrow-up"></i>';
    initIcons();
  }
}

// ─── テキストエリアの高さ自動調整 ────────────────────────────────────────────
if (input) {
  input.addEventListener("input", () => {
    input.style.height = "auto";
    input.style.height = `${Math.min(input.scrollHeight, 120)}px`;
  });

  // Enter 送信 / Shift+Enter 改行
  input.addEventListener("keydown", (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      form?.requestSubmit();
    }
  });
}

form?.addEventListener("submit", async (e: SubmitEvent) => {
  e.preventDefault();
  const text = input?.value.trim() ?? "";
  if (!text || !input) return;

  const active = CharacterStore.getActive();
  if (!active) {
    appendMessage("assistant", "キャラクターが選択されていません。先にキャラクターを選んでください。", false);
    return;
  }

  // 空状態を消してから追加する
  const emptyEl = messages?.querySelector(".chat__empty");
  if (emptyEl) emptyEl.remove();

  appendMessage("user", text, false);
  input.value = "";
  input.style.height = "auto";

  setLoading(true);
  const pending = appendMessage("assistant", "", true); // ローディング表示

  try {
    const { text: reply, saved } = await sendChatMessage(active, [...conversation], text);
    // 応答は取得できた → 必ず表示し会話へ反映する（保存可否に関わらず、QA-R1）。
    const bubbleEl = pending.querySelector<HTMLElement>(".mtt-bubble");
    if (bubbleEl) {
      bubbleEl.classList.remove("mtt-typing");
      bubbleEl.innerHTML = "";
      bubbleEl.textContent = reply;
    }
    conversation.push({ role: "user", content: text });
    conversation.push({ role: "assistant", content: reply });
    if (!saved) {
      // 履歴保存だけ失敗した場合は、応答は活かしつつ控えめに注記する。
      if (bubbleEl) {
        bubbleEl.title = "この応答の履歴保存に失敗しました（会話は継続できます）。";
        bubbleEl.classList.add("msg--unsaved");
      }
    }
  } catch (error) {
    // ModelError は { kind, message }。キー未設定は設定へ誘導する（要件3.4）。エラー時は履歴を残さない（6.2）。
    const kind = (error as { kind?: string })?.kind;
    const bubbleEl = pending.querySelector<HTMLElement>(".mtt-bubble");
    if (bubbleEl) {
      bubbleEl.classList.remove("mtt-typing");
      bubbleEl.innerHTML = "";
      if (kind === "ApiKeyMissing") {
        bubbleEl.textContent =
          "選択中モデルの API キーが未設定です。上部の設定パネルで API キーを設定してください。";
      } else {
        bubbleEl.textContent = `送信に失敗しました: ${(error as { message?: string })?.message ?? error}`;
      }
      bubbleEl.classList.add("msg--error");
    }
    console.error(error);
  } finally {
    setLoading(false);
  }
});

/**
 * メッセージバブルを messages エリアへ追加する。
 * 原則8: assistant ロールには必ず AI ラベル（.mtt-ai-label）を添える。
 */
function appendMessage(role: ChatRole, text: string, isLoading: boolean): HTMLDivElement {
  const wrapper = document.createElement("div");
  wrapper.className = `mtt-msg mtt-msg--${role === "user" ? "user" : "assistant"}`;

  // アバター
  const avatar = document.createElement("div");
  avatar.className = `mtt-avatar mtt-avatar--${role === "user" ? "me" : "ai"}`;
  avatar.innerHTML = role === "user"
    ? '<i data-lucide="user"></i>'
    : '<i data-lucide="sparkles"></i>';

  // バブルラップ
  const bubbleWrap = document.createElement("div");
  bubbleWrap.className = "mtt-bubble-wrap";

  // AI の場合はメタ行に AI ラベルを追加（原則8）
  if (role === "assistant") {
    const meta = document.createElement("div");
    meta.className = "mtt-meta";
    // AI ラベル（原則8: 必須）
    const aiLabel = document.createElement("span");
    aiLabel.className = "mtt-ai-label";
    aiLabel.innerHTML = '<i data-lucide="bot"></i>AI';
    meta.appendChild(aiLabel);
    bubbleWrap.appendChild(meta);
  }

  // バブル本体
  const bubble = document.createElement("div");
  bubble.className = `mtt-bubble mtt-bubble--${role === "user" ? "me" : "ai"}`;

  if (isLoading) {
    bubble.classList.add("mtt-typing");
    bubble.innerHTML = "<span></span><span></span><span></span>";
  } else {
    bubble.textContent = text;
  }

  bubbleWrap.appendChild(bubble);
  wrapper.append(avatar, bubbleWrap);
  messages?.appendChild(wrapper);
  if (messages) messages.scrollTop = messages.scrollHeight;

  initIcons();
  return wrapper;
}
