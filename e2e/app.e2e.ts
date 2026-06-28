// Tauri アプリの最小 E2E スモーク（動作確認済み・`pnpm e2e` で PASS）。
// 実アプリを起動し、主要 UI（チャット入力・モデルパネル・キャラクターパネル）が存在することを確認する。

describe("Mitatete アプリ起動スモーク", () => {
  it("チャット UI と主要パネルが表示される", async () => {
    // チャット入力欄。
    const input = await $("#input");
    await expect(input).toBeExisting();

    // モデル選択パネル・キャラクター選択パネルのマウント点。
    await expect(await $("#model-panel")).toBeExisting();
    await expect(await $("#character-panel")).toBeExisting();

    // 原則8（固定）の明示が表示されている。
    const disclosure = await $(".chat__disclosure");
    await expect(disclosure).toBeExisting();
  });
});
