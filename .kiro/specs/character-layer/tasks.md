# 実装計画

## タスク一覧

- [x] 1. 基盤：データ型定義とバリデーション
- [x] 1.1 CharacterSchema 型定義とバリデーターの実装
  - `CharacterSchema` および `VisualConfig` の型定義を `character-validator.ts` に記述する（TypeScript の `interface` を使用）
  - `aiDisclosure` の固定文言（「私はAIアシスタントです。人間ではありません。」）を定数として定義し、いかなる入力でも上書きできないよう強制付与するロジックを実装する
  - `name`・`tone` の非空チェックを実装し、違反時は例外をスローする
  - `validate()` が返す `CharacterSchema.aiDisclosure` が常に固定文言と一致することをユニットテストで確認する
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 2. バックエンド：Tauriコマンドの実装
- [x] 2.1 storage.rs へのキャラクター保存・読み込み・削除コマンド追加
  - `save_character(schema_json: String) -> Result<(), String>` を実装し、`~/.mitatete/characters/{id}.json` へ書き込む
  - `load_characters() -> Result<Vec<String>, String>` を実装し、`~/.mitatete/characters/` 配下の全JSONを読み込んで返す
  - `delete_character(id: String) -> Result<(), String>` を実装し、対象ファイルを削除する
  - `save_character` は同一IDで呼ばれた場合に既存ファイルを上書きする（冪等性）
  - `tauri.conf.json` にコマンド権限を追加し、フロントエンドから `invoke` できることを確認する
  - _Requirements: 2.5, 5.1, 5.2, 5.3_

- [x] 3. コア：キャラクター状態管理
- [x] 3.1 character-store.ts の実装
  - `init()` でアプリ起動時にTauriコマンド `load_characters` を呼び出し、保存済みキャラクターを復元する
  - `load_characters` 失敗時はデフォルトキャラクター（プリセット第一候補）にフォールバックし、エラーをUIに通知する
  - `setActive(id)` を実装し、アクティブキャラクター変更を購読者（原則エンジン・キャラクターウィンドウ）に通知する
  - `setActive()` が内部タイマーやAIレスポンスから呼ばれないよう、呼び出し元を明示的にUIイベントハンドラーに限定する設計コメントを付与する
  - `subscribe(listener)` で変更通知を受け取れることをユニットテストで確認する
  - _Requirements: 3.3, 4.1, 4.2, 4.3, 5.2, 5.3, 5.4_

- [x] 4. コア：プリセットキャラクター読み込みUI
- [x] 4.1 (P) プリセット定義ファイルの作成と読み込み処理
  - `public/presets/preset-a.json` を `CharacterSchema` 準拠の形式で作成する（最低1件）
  - `character-ui.ts` で `public/presets/*.json` を fetch して一覧データを構築するロジックを実装する
  - プリセット定義ファイルが存在しない場合のエラー通知処理を実装する
  - プリセット一覧が画面に表示され、選択できることを手動確認できる状態にする
  - _Requirements: 1.1, 1.4, 1.5_
  - _Boundary: character-ui.ts_

- [x] 4.2 (P) プリセット選択時のCharacterSchema生成フロー
  - `character-ui.ts` でプリセット選択イベントを受け取り、`character-validator.ts` を通じて `CharacterSchema` を生成するフローを実装する
  - `character-validator.ts` が `aiDisclosure` を固定付与した後、`character-store.ts` の `save` と `setActive` を順に呼び出す
  - プリセットを選択するとTauriコマンド経由で保存され、アクティブキャラクターが更新されることを確認する
  - _Requirements: 1.2, 1.3_
  - _Boundary: character-ui.ts, character-validator.ts_
  - _Depends: 4.1_

- [x] 5. コア：カスタムキャラクター作成UI
- [x] 5.1 カスタムキャラクター作成フォームの実装
  - `character-editor.ts` に名前・口調・ビジュアル（画像アップロード）の入力フォームを実装する
  - ビジュアルが未設定の場合、デフォルトアバター（内蔵SVG）を `visual` フィールドに自動設定する
  - `aiDisclosure` フィールドをフォームのUIに表示しない（または読み取り専用テキストで表示し、編集不可にする）
  - 保存ボタン押下時に `character-validator.ts` → `character-store.ts` 経由で保存が完了し、`~/.mitatete/characters/{id}.json` が生成されることを確認する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_
  - _Boundary: character-editor.ts_

- [x] 6. 統合：キャラクター切り替えと下流通知
- [x] 6.1 切り替えUIと原則エンジン・キャラクターウィンドウへの通知統合
  - `character-ui.ts` にキャラクター切り替えボタン／セレクターを実装し、`character-store.setActive()` を呼び出す
  - `character-store.ts` の `subscribe` を使い、`principles.ts`（原則エンジン）が `principleDefaults` を受け取って更新することを確認する
  - `character-store.ts` の `subscribe` を使い、`character.ts`（キャラクターウィンドウ）が `visual` と `name` を受け取って表示を更新することを確認する
  - キャラクターを切り替えたとき、原則エンジンとキャラクターウィンドウの両方が即座に更新されることを手動で確認できる状態にする
  - _Requirements: 4.1, 4.4_
  - _Depends: 3.1, 4.2, 5.1_

- [x] 7. 検証：起動時復元とエラー縮退の確認
- [x] 7.1 アプリ再起動後のキャラクター復元テスト
  - カスタムキャラクターを保存後にアプリを再起動し、最後に使用したキャラクターが正しく復元されることを確認する
  - `~/.mitatete/characters/` が存在しない初回起動時にデフォルトキャラクターで正常起動することを確認する
  - ファイルが破損している場合（不正JSON）にフォールバック動作が発動することを確認する
  - _Requirements: 5.2, 5.3, 5.4_

- [x] 8. フェーズ2：ビジュアルエディター（コア完成後に着手）
- [x] 8.1 (P) VisualConfig レイヤー構造エディターの実装
  - `character-visual-editor.ts` に体型・目・髪・服の色・肌色のレイヤーを選択できるUIを実装する
  - 選択内容をリアルタイムでSVGプレビューに反映する
  - 設定を `VisualConfig`（mode: 'template'）として `CharacterSchema.visualConfig` に格納し、`character-editor.ts` から呼び出せることを確認する
  - _Requirements: 6.1, 6.2_
  - _Boundary: character-visual-editor.ts_

- [x] 8.2 (P) 自作画像アップロードと著作権同意フローの実装
  - PNG/SVGファイルのアップロード受け付け処理を実装する
  - アップロード前に著作権注意文（「既存のアニメ・ゲーム・商標キャラクターに似せた画像のアップロードは著作権侵害になる場合があります」）を表示し、同意確認ダイアログを実装する
  - ユーザーが同意した場合のみアップロードを進め、`VisualConfig`（mode: 'upload', uploadedImagePath）として格納する
  - 同意拒否時はアップロードをキャンセルし、既存のビジュアル設定を維持することを確認する
  - _Requirements: 6.3, 6.4, 6.5_
  - _Boundary: character-visual-editor.ts_

## Implementation Notes

- 1.1: `src/character-validator.ts` に CharacterSchema/VisualConfig 型・`AI_DISCLOSURE` 定数・`validate()`（aiDisclosure 強制付与=上書き不可、name/tone 非空チェック）を実装。フロントのテスト基盤として **vitest** を導入（`pnpm test`=`vitest run`）。
- 2.1(調整): design の `save_character(schema_json)` は storage-manager 既存の `save_character(name,data)` コマンドを**再利用**して満たす（コマンド名衝突回避。フロントは name=id, data=schema で呼ぶ）。新規追加は `delete_character`（ローカルのみ・サニタイズ・冪等。GDrive削除は storage 要件4.2 で禁止のため行わない）と `load_characters`（全キャラの**フルJSON**を Vec で返す。破損ファイルは起動時復元のためスキップ）。両コマンドを lib.rs に登録。`list_characters`(名前一覧)とは別物。
- 3.1: `src/character-store.ts` に CharacterStore（init/getActive/getAll/setActive/save/delete/subscribe）をモジュール内 state（Map+activeId+listeners）で実装。invoke は `@tauri-apps/api/core`。プリセット fetch（4.1の責務）が未実装のため、init 失敗・0件時の縮退用に**内蔵 `DEFAULT_CHARACTER`** を1件定義（TODO(4.1)で差し替え）。save/init の復元時に `CharacterValidator.validate` を再適用し aiDisclosure 不変条件を二重担保。`setActive` はユーザー操作起点限定の設計コメントを付与（実行時強制はしない）。テストは `vi.mock("@tauri-apps/api/core")` で invoke をモック（store 10 + validator 9 = 19 pass）。
- 4.1(調整): 実行時に `public/presets/*.json` をグロブ列挙する手段が無いため、読み込み対象を列挙した**マニフェスト `public/presets/index.json`** を起点に各定義を fetch する方式を採用。`src/character-ui.ts` に `loadPresets`（マニフェスト失敗→空配列+通知、個別定義の欠損→部分縮退）・`renderPresetList`（クリック選択+`.is-selected`ハイライト）・`initCharacterUI`（`#character-panel` へ描画・エラーはパネル内表示）を実装。プリセットは `preset-a.json`/`preset-b.json` の2件。選択→validate→store の配線は**4.2の責務**のため `onSelect` はプレースホルダ（TODO(4.2)）。必要な足場として index.html に `#character-panel`+スクリプト追加・styles.css に `.character-panel*` を追加。テストは `vi.stubGlobal("fetch")` で fetch をモック（ui 4 pass、計23 pass）。一覧の画面表示・選択操作は `pnpm dev`（Tauri webview）での手動確認が必要。
- 4.2: `src/character-ui.ts` に `selectPreset`（候補→`CharacterValidator.validate` で aiDisclosure 固定付与→`CharacterStore.save`（Tauriコマンド永続化）→`CharacterStore.setActive`（アクティブ化・下流通知）の順）を実装し、`initCharacterUI` の既定 `onSelect` に配線（4.1のプレースホルダを差し替え）。validate 失敗時は保存せずエラー通知。`selectPreset` は setActive の「ユーザー操作起点限定」を満たす正規経路。テストは invoke（store経由）と fetch を併せてモック（selectPreset 3 + 計26 pass）。
- 5.1: `src/character-editor.ts` に `buildCustomCharacter`（visual未設定→内蔵 `DEFAULT_AVATAR`(SVG data URI) 適用・validate で aiDisclosure 固定付与・isPreset=false）・`submitCustomCharacter`（save のみ。**アクティブ化はしない**＝design フロー2 と一致、切り替えは6.1）・`initCharacterEditor`（名前/口調/画像file入力・aiDisclosure を読み取り専用テキストで表示=編集不可）を実装。画像は file→data URL 取り込み。著作権同意フロー・VisualConfig(upload) はフェーズ2（8.2）の責務として未実装。index.html に `#character-editor`+スクリプト、styles.css に `.editor*` 追加。テスト8件（計34 pass）。フォーム操作の実画面確認は `pnpm dev` が必要。
- 6.1: main/character は**別ウィンドウ＝別webview＝別JSコンテキスト**のため、store は跨げない。原則エンジン（同一window）は store を直接 subscribe、character ウィンドウ（別window）は **Tauri イベント `character:changed`** で受信（`core:default` 権限で emit/listen 可）。`principles.ts` に `initPrincipleEngine`/`getCurrentPrinciples`（store購読→principleDefaults更新）、`character-ui.ts` に `switchCharacter`（setActive正規経路）・`connectCrossWindow`（store購読→emit放送）・`renderSwitcher`（getAll()の`<select>`）を追加、`initCharacterUI` で store.init→セレクター描画→store変更で再描画。`character.ts` に `listen("character:changed")`→`updateCharacterDisplay`（visual=img/name=title）を追加。index.html に principles.ts を読込。テストは invoke と emit を併せてモック（principles 2 + ui切替 2、計38 pass）。両ウィンドウ即時更新の通し確認は `pnpm dev`（マルチウィンドウ）が必要。
- 7.1: 検証中に要件5.2「**最後に使用した**キャラクター復元」が未充足（init が先頭固定）と判明したため、`character-store.ts` に `lastActiveId` 永続化を実装してギャップを閉じた。永続化先は storage-manager の `save_settings`/`read_settings`（`~/.mitatete/settings.json`）を再利用し、キー `lastActiveCharacterId` を read→merge→save（他設定を保持）。setActive で保存、init で読み出し（記録IDが復元集合に無ければ先頭フォールバック）。検証は store ユニットテストで網羅: 再起動復元・記録欠落フォールバック・設定マージ保存・不正JSONスキップ/全破損フォールバック・初回(0件)デフォルト（計43 pass）。`vite build` 成功（index/character 両ページ）。**残: 実機 `pnpm tauri dev` でカスタム作成→再起動→復元の目視確認（M2 サインオフ）は GUI 必須のため人手で実施が必要。**
- 8.1/8.2: `src/character-visual-editor.ts` を新規実装。8.1=`buildTemplateVisualConfig`/`buildVisualSvg`（体型・目・髪・服色・肌色をレイヤー化したパラメトリックSVG）/`svgToDataUri`/`initVisualEditor`（選択UI＋リアルタイムプレビュー、`VisualConfig(mode:'template')` 収集）。8.2=`requestImageUpload`（PNG/SVG限定→`COPYRIGHT_NOTICE` 同意ゲート→同意で `VisualConfig(mode:'upload', uploadedImagePath)`、拒否/非対応は既存設定維持）。同意取得(`ConsentPrompt`)とパス解決(`PathResolver`)は注入可能（テスト容易・Tauri dialog 実パスへ差し替え余地）。`character-editor.ts` に統合: `CustomCharacterInput.visualConfig` 追加、`buildCustomCharacter` が template→SVG data URI / upload→uploadedImagePath を visual に導出。フォームにエディター＋アップロード(window.confirm 同意)を配線。テスト13件（visual-editor 11 + editor 2、計56 pass）。**繰越: Tauri dialog プラグイン未導入のため uploadedImagePath は現状ファイル名。実ローカルパス取得は dialog 導入後の follow-up。**
