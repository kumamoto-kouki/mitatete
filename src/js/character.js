// キャラクターウィンドウの描画・独り言（スタブ）。
// character-layer spec の実装時に、CharacterSchema からビジュアル/口調を反映する。
// クリックで main（チャット）ウィンドウを前面に出す。

const character = document.querySelector("#character");

character?.addEventListener("click", async () => {
  // withGlobalTauri: true のため window.__TAURI__ が利用可能
  try {
    const { Window } = window.__TAURI__.window;
    await Window.getByLabel("main")?.then((w) => w?.setFocus());
  } catch {
    // 開発中（ブラウザ単体表示など）は no-op
  }
});
