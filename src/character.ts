// キャラクターウィンドウの描画・独り言（スタブ）。
// character-layer spec の実装時に、CharacterSchema からビジュアル/口調を反映する。
// クリックで main（チャット）ウィンドウを前面に出す。
import { Window } from "@tauri-apps/api/window";

const character = document.querySelector<HTMLDivElement>("#character");

character?.addEventListener("click", async () => {
  const main = await Window.getByLabel("main");
  await main?.setFocus();
});
