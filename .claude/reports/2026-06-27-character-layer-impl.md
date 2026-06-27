# 2026-06-27 character-layer 実装の振り返り（コア 3.1〜7.1）

## 概要

character-layer spec のコア＋検証タスク（3.1 / 4.1 / 4.2 / 5.1 / 6.1 / 7.1）をメインセッションの逐次実装で完遂。フロントエンドは TypeScript（vanilla）＋ vitest。**43 ユニットテスト pass**、`tsc --noEmit` クリーン、`vite build` 成功。残るは 8.1/8.2（フェーズ2 ビジュアルエディター）のみ。

実機 `pnpm tauri dev` を WSLg 上で起動し、**プリセット選択→保存→アクティブ化→`lastActiveCharacterId` 永続化が実アプリで動作**することを on-disk（`~/.mitatete/characters/`・`settings.json`）で確認した。

## うまくいったこと

- **store を唯一の権威ソースに集約**: `character-store.ts`（Map+activeId+listeners）を起点に、validator・UI・editor・principles・character window がぶら下がる単純な放射状構造。テストは全経路 `vi.mock("@tauri-apps/api/core")` で invoke を差し替え、ファイルI/Oに依存せず検証できた。
- **不変条件の二重担保**: `aiDisclosure` は validator が固定付与し、store の save/init でも再 validate。改ざんJSONを復元しても固定文言に戻ることをテストで保証（原則8）。
- **境界の明確な分割**: 4.1（読み込み）/4.2（選択フロー）/5.1（作成）/8.2（著作権同意）をファイル境界で分け、各タスクで `onSelect` プレースホルダ→本実装の差し替えを段階的に行えた。`submitCustomCharacter` は save のみ（setActive しない）で design フロー2 と一致させ、テストの invoke 回数で境界を固定。
- **クロスウィンドウ通知の正しい分離**: main/character が別 webview＝別 JS コンテキストである事実に気づき、原則エンジン（同一window）は store 直接 subscribe、character window（別window）は Tauri イベント `character:changed` の emit/listen に分けた。混同するとサイレントに動かないバグになっていた。

## ハマりどころ / 設計判断

- **`public/presets/*.json` のグロブ不可**: 実行時にディレクトリ glob はできないため、`public/presets/index.json` マニフェスト方式に変更（design の `*.json` 表記を実装で具体化）。Implementation Notes に明記。
- **要件5.2「最後に使用したキャラクター」のギャップ**: 7.1（検証）に着手して初めて、`init()` が先頭固定で「最後に使用した」を満たしていないと判明。`save_settings`/`read_settings`（storage-manager）を再利用し `lastActiveCharacterId` を read→merge→save で永続化して閉じた。**検証タスクが実装ギャップを発見する好例**——7.x を「ただの確認」と軽視しない。
- **console.info は端末転送されない**: Tauri dev は webview の console.error を端末へ転送するが info/log は出さない。起動時アクティブIDの観測に info を足したがログに出ず、on-disk 状態＋ユニットテストの合成で復元を実証した。診断ログ自体は devtools で有用なため残置。
- **keyring エラーはノイズ**: 実機起動時の `OAuthFailed: No default store has been set` は storage-manager の keyring が headless env で動かないだけで character-layer とは無関係。

## 繰越事項（follow-up）

- **M2 サインオフ（2026-06-27 確認済み）**: 実機 `pnpm tauri dev` で、アクティブキャラクター（seed した「ミタ太郎」）を起動時に store が復元 → `character:changed` emit → **別 webview のキャラクターウィンドウが受信・描画**する end-to-end を、`updateCharacterDisplay` に一時診断（転送される console.error）を入れてログで実証（`[M2-DIAG] character window displayed: ミタ太郎`）。診断は確認後に撤去。スクリーンショットツールが env に無いため、ピクセル単位の目視は WSLg で人間が確認可（窓は表示済み）。
- **switcher のプリセット網羅**: 切り替えセレクターは `store.getAll()`（=保存済み）を列挙するため、未選択のプリセットは出ない。プリセットを常に選択肢へ出すなら UI 集約が要る（現状はプリセットパネルで選ぶと switcher に入る）。
- **フェーズ2（8.1/8.2）**: `VisualConfig` レイヤーエディターと著作権同意フローは未着手。`character-visual-editor.ts` 境界で実装する。
- **WSLg で Tauri GUI は起動可能**（webkit2gtk-4.1 あり）。storage-manager レポートの「GUI ランタイム未検証」はこの env では解消。GPU は zink/libEGL 警告でソフトレンダにフォールバック。

## 体制メモ

- GitHub Project 反映漏れの指摘を受け、**着手時 In Progress / 完了時 Done** を徹底する運用に明文化（[[github-project-ids]]）。今回 3.1〜7.1 と親エピック3〜7 を Done に同期。
- tasks.md の `[x]` 更新と GitHub Project 更新はセットで行う。
