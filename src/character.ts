// キャラクターウィンドウの描画・独り言。
// CharacterSchema の visual / name を反映する。クリックで main（チャット）ウィンドウを前面に出す。
//
// 注: このウィンドウは main とは別 webview のため CharacterStore を直接参照できない。
// アクティブキャラクター変更は main ウィンドウから Tauri イベント（character:changed）で受信する。(要件 4.1, 4.4)

import { Window } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import type { CharacterSchema } from "./character-validator";
import { applyTheme, type Theme } from "./theme";

const CHARACTER_CHANGED_EVENT = "character:changed";

const character = document.querySelector<HTMLDivElement>("#character");

character?.addEventListener("click", async () => {
  const main = await Window.getByLabel("main");
  await main?.setFocus();
});

/** 受信した CharacterSchema からビジュアル・名前の表示を更新する。(要件 4.1, 4.4) */
export function updateCharacterDisplay(schema: CharacterSchema): void {
  if (!character) return;

  // 名前はツールチップ・読み上げラベルに反映する。
  character.title = schema.name;
  character.setAttribute("aria-label", schema.name);

  const placeholder =
    character.querySelector<HTMLElement>(".character__placeholder");
  if (!placeholder) return;

  if (schema.visual && schema.visual.trim() !== "") {
    // ビジュアル指定があれば画像で表示する。
    placeholder.replaceChildren();
    const img = document.createElement("img");
    img.className = "character__image";
    img.src = schema.visual;
    img.alt = schema.name;
    placeholder.appendChild(img);
  } else {
    // 未設定ならプレースホルダ記号に戻す。
    placeholder.textContent = "◌";
  }
}

// main ウィンドウからのアクティブキャラクター変更を受信して表示を更新する。
void listen<CharacterSchema>(CHARACTER_CHANGED_EVENT, (event) => {
  updateCharacterDisplay(event.payload);
});

// main ウィンドウでのテーマ切替をリアルタイムで反映する（W-1）。
void listen<Theme>("theme:changed", (event) => {
  applyTheme(event.payload);
});
