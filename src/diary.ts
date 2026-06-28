// diary.ts — 日記エンジン（原則 9「観察を記述する、評価しない」）
//
// 生成フロー:
//   1. character-store.getActive() → diaryEnabled チェック（要件 1.2）
//   2. calcDiaryIntensity → detailLevelFromIntensity で詳細度決定（要件 2.1, 2.2）
//   3. read_history(今日) → 空なら非生成（要件 3.1, 3.2）
//   4. buildDiaryPrompt(name, detail) を system に、当日履歴を messages として generate_text 呼び出し（要件 4）
//   5. 成功時のみ表示 + save_diary 依頼（要件 5.1, 5.2, 5.4）
//   6. 生成失敗時は失敗メッセージ表示、save_diary 呼ばない（要件 6.1, 6.2）
//
// トリガー: ユーザー操作（生成ボタン）のみ。タイマー・自動実行を持たない（要件 1.3, 2.3）。

import { invoke } from "@tauri-apps/api/core";
import { CharacterStore } from "./character-store";
import { calcDiaryIntensity } from "./principles";
import { buildDiaryPrompt, detailLevelFromIntensity } from "./diary-prompt";

// ─── 型定義 ───────────────────────────────────────────────────────────────────

/** 生成結果の discriminated union。 */
export type DiaryResult =
  | { status: "disabled" }
  | { status: "no_history" }
  | { status: "ok"; content: string; saved: boolean }
  | { status: "error"; message: string };

/** 当日の日付（YYYY-MM-DD）。storage の save_diary が要求する形式。 */
export function todayDateString(date: Date = new Date()): string {
  return date.toISOString().slice(0, 10);
}

// ─── 日記生成オーケストレーション ───────────────────────────────────────────

/**
 * 当日の観察日記を生成する。
 *
 * ユーザー操作（生成ボタン）起点でのみ呼び出すこと。
 * タイマーや AI レスポンスからの自動起動は禁止（要件 1.3, 2.3）。
 *
 * @returns DiaryResult — 各状態の理由・内容を含む discriminated union
 */
export async function generateTodaysDiary(): Promise<DiaryResult> {
  // Step 1: アクティブキャラクターと原則 9 ON/OFF 確認（要件 1.2）
  const active = CharacterStore.getActive();
  if (active === null || !active.diaryEnabled) {
    return { status: "disabled" };
  }

  // Step 2: 強度導出 → 詳細度決定（要件 2.1, 2.2, 2.3）
  const intensity = calcDiaryIntensity(active.principleDefaults);
  const detail = detailLevelFromIntensity(intensity);

  // Step 3: 当日履歴の収集（要件 3.1, 3.3）
  const today = todayDateString();
  let history: unknown;
  try {
    history = await invoke<unknown>("read_history", { date: today });
  } catch (_err) {
    // read_history がファイル未存在等で失敗した場合は空と同等扱い
    history = null;
  }

  // 履歴が空・null・空配列ならば非生成（要件 3.2）
  if (
    history === null ||
    history === undefined ||
    (Array.isArray(history) && history.length === 0)
  ) {
    return { status: "no_history" };
  }

  // Step 4: 日記生成プロンプト構築 → generate_text 呼び出し（要件 4.1〜4.4）
  const systemPrompt = buildDiaryPrompt(active.name, detail);
  const historyJson = JSON.stringify(history);

  let generatedText: string;
  try {
    generatedText = await invoke<string>("generate_text", {
      systemPrompt,
      historyJson,
    });
  } catch (err) {
    // 生成失敗時：save_diary を呼ばない（要件 6.1, 6.2）
    const message =
      err !== null && typeof err === "object" && "message" in err
        ? String((err as { message: unknown }).message)
        : "日記の生成に失敗しました。";
    console.error("[diary] generate_text 失敗:", err);
    return { status: "error", message };
  }

  // Step 5: 生成成功 → 内容改変なしで save_diary 依頼（要件 5.2, 5.4）
  // 保存失敗は生成結果を取り消さない（フロントエンドルール：応答の成功と保存の成功を分離）
  let saved = true;
  try {
    await invoke("save_diary", { date: today, content: generatedText });
  } catch (err) {
    saved = false;
    console.error("[diary] save_diary 失敗（生成テキストは保持）:", err);
  }

  // Step 6: 生成テキストを返す（表示は UI ハンドラが行う、要件 5.1）
  return { status: "ok", content: generatedText, saved };
}

// ─── UI 配線（DOM ハンドラ）──────────────────────────────────────────────────

/**
 * 日記パネルを DOM に配線する。
 *
 * `#diary-panel` に生成ボタン・本文表示領域・通知行を追加し、
 * ボタンクリックで `generateTodaysDiary` を起動する。
 *
 * ユーザー操作起点のみ（要件 1.3, 2.3）。
 */
export function initDiaryPanel(): void {
  const panel = document.querySelector<HTMLElement>("#diary-panel");
  if (!panel) return;

  // マウント点を構築する
  panel.innerHTML = `
    <div class="diary-panel__inner">
      <h2 class="diary-panel__title">観察日記（原則 9）</h2>
      <button id="diary-generate-btn" class="diary-panel__btn" type="button">
        今日の日記を生成
      </button>
      <p id="diary-notice" class="diary-panel__notice" aria-live="polite"></p>
      <div id="diary-content" class="diary-panel__content" aria-label="生成された観察日記"></div>
    </div>
  `;

  const btn = panel.querySelector<HTMLButtonElement>("#diary-generate-btn");
  const noticeEl = panel.querySelector<HTMLParagraphElement>("#diary-notice");
  const contentEl = panel.querySelector<HTMLDivElement>("#diary-content");

  if (!btn || !noticeEl || !contentEl) return;

  btn.addEventListener("click", async () => {
    btn.disabled = true;
    noticeEl.textContent = "生成中…";
    contentEl.textContent = "";

    try {
      const result = await generateTodaysDiary();

      switch (result.status) {
        case "disabled":
          // 要件 1.2: 原則 9 OFF の理由を通知行に表示
          noticeEl.textContent =
            "観察日記（原則 9）が無効になっています。キャラクター設定から有効にしてください。";
          break;

        case "no_history":
          // 要件 3.2: 対象履歴がない旨を表示
          noticeEl.textContent =
            "今日の対話履歴がありません。対話後に再度お試しください。";
          break;

        case "ok":
          // 要件 5.1: 生成された観察日記を表示（内容改変なし、要件 5.4）
          contentEl.textContent = result.content;
          noticeEl.textContent = result.saved
            ? "保存しました。"
            : "生成しました（保存に失敗しました）。";
          break;

        case "error":
          // 要件 6.1: 失敗メッセージを表示
          noticeEl.textContent = `日記の生成に失敗しました: ${result.message}`;
          break;
      }
    } catch (err) {
      noticeEl.textContent = "予期しないエラーが発生しました。";
      console.error("[diary] initDiaryPanel 予期しないエラー:", err);
    } finally {
      btn.disabled = false;
    }
  });
}

// ─── 起動フック ───────────────────────────────────────────────────────────────

// main ウィンドウ読み込み時に日記パネルを配線する。
// DOM がない環境（テスト等）は document チェックで skip する。
if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", initDiaryPanel);
  } else {
    initDiaryPanel();
  }
}
