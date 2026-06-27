# 実装計画

## タスク一覧

- [ ] 1. 基盤：データ型定義とバリデーション
- [x] 1.1 CharacterSchema 型定義とバリデーターの実装
  - `CharacterSchema` および `VisualConfig` の型定義を `character-validator.ts` に記述する（TypeScript の `interface` を使用）
  - `aiDisclosure` の固定文言（「私はAIアシスタントです。人間ではありません。」）を定数として定義し、いかなる入力でも上書きできないよう強制付与するロジックを実装する
  - `name`・`tone` の非空チェックを実装し、違反時は例外をスローする
  - `validate()` が返す `CharacterSchema.aiDisclosure` が常に固定文言と一致することをユニットテストで確認する
  - _Requirements: 3.1, 3.2, 3.3_

- [ ] 2. バックエンド：Tauriコマンドの実装
- [x] 2.1 storage.rs へのキャラクター保存・読み込み・削除コマンド追加
  - `save_character(schema_json: String) -> Result<(), String>` を実装し、`~/.mitatete/characters/{id}.json` へ書き込む
  - `load_characters() -> Result<Vec<String>, String>` を実装し、`~/.mitatete/characters/` 配下の全JSONを読み込んで返す
  - `delete_character(id: String) -> Result<(), String>` を実装し、対象ファイルを削除する
  - `save_character` は同一IDで呼ばれた場合に既存ファイルを上書きする（冪等性）
  - `tauri.conf.json` にコマンド権限を追加し、フロントエンドから `invoke` できることを確認する
  - _Requirements: 2.5, 5.1, 5.2, 5.3_

- [ ] 3. コア：キャラクター状態管理
- [ ] 3.1 character-store.ts の実装
  - `init()` でアプリ起動時にTauriコマンド `load_characters` を呼び出し、保存済みキャラクターを復元する
  - `load_characters` 失敗時はデフォルトキャラクター（プリセット第一候補）にフォールバックし、エラーをUIに通知する
  - `setActive(id)` を実装し、アクティブキャラクター変更を購読者（原則エンジン・キャラクターウィンドウ）に通知する
  - `setActive()` が内部タイマーやAIレスポンスから呼ばれないよう、呼び出し元を明示的にUIイベントハンドラーに限定する設計コメントを付与する
  - `subscribe(listener)` で変更通知を受け取れることをユニットテストで確認する
  - _Requirements: 3.3, 4.1, 4.2, 4.3, 5.2, 5.3, 5.4_

- [ ] 4. コア：プリセットキャラクター読み込みUI
- [ ] 4.1 (P) プリセット定義ファイルの作成と読み込み処理
  - `public/presets/preset-a.json` を `CharacterSchema` 準拠の形式で作成する（最低1件）
  - `character-ui.ts` で `public/presets/*.json` を fetch して一覧データを構築するロジックを実装する
  - プリセット定義ファイルが存在しない場合のエラー通知処理を実装する
  - プリセット一覧が画面に表示され、選択できることを手動確認できる状態にする
  - _Requirements: 1.1, 1.4, 1.5_
  - _Boundary: character-ui.ts_

- [ ] 4.2 (P) プリセット選択時のCharacterSchema生成フロー
  - `character-ui.ts` でプリセット選択イベントを受け取り、`character-validator.ts` を通じて `CharacterSchema` を生成するフローを実装する
  - `character-validator.ts` が `aiDisclosure` を固定付与した後、`character-store.ts` の `save` と `setActive` を順に呼び出す
  - プリセットを選択するとTauriコマンド経由で保存され、アクティブキャラクターが更新されることを確認する
  - _Requirements: 1.2, 1.3_
  - _Boundary: character-ui.ts, character-validator.ts_
  - _Depends: 4.1_

- [ ] 5. コア：カスタムキャラクター作成UI
- [ ] 5.1 カスタムキャラクター作成フォームの実装
  - `character-editor.ts` に名前・口調・ビジュアル（画像アップロード）の入力フォームを実装する
  - ビジュアルが未設定の場合、デフォルトアバター（内蔵SVG）を `visual` フィールドに自動設定する
  - `aiDisclosure` フィールドをフォームのUIに表示しない（または読み取り専用テキストで表示し、編集不可にする）
  - 保存ボタン押下時に `character-validator.ts` → `character-store.ts` 経由で保存が完了し、`~/.mitatete/characters/{id}.json` が生成されることを確認する
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5_
  - _Boundary: character-editor.ts_

- [ ] 6. 統合：キャラクター切り替えと下流通知
- [ ] 6.1 切り替えUIと原則エンジン・キャラクターウィンドウへの通知統合
  - `character-ui.ts` にキャラクター切り替えボタン／セレクターを実装し、`character-store.setActive()` を呼び出す
  - `character-store.ts` の `subscribe` を使い、`principles.ts`（原則エンジン）が `principleDefaults` を受け取って更新することを確認する
  - `character-store.ts` の `subscribe` を使い、`character.ts`（キャラクターウィンドウ）が `visual` と `name` を受け取って表示を更新することを確認する
  - キャラクターを切り替えたとき、原則エンジンとキャラクターウィンドウの両方が即座に更新されることを手動で確認できる状態にする
  - _Requirements: 4.1, 4.4_
  - _Depends: 3.1, 4.2, 5.1_

- [ ] 7. 検証：起動時復元とエラー縮退の確認
- [ ] 7.1 アプリ再起動後のキャラクター復元テスト
  - カスタムキャラクターを保存後にアプリを再起動し、最後に使用したキャラクターが正しく復元されることを確認する
  - `~/.mitatete/characters/` が存在しない初回起動時にデフォルトキャラクターで正常起動することを確認する
  - ファイルが破損している場合（不正JSON）にフォールバック動作が発動することを確認する
  - _Requirements: 5.2, 5.3, 5.4_

- [ ] 8. フェーズ2：ビジュアルエディター（コア完成後に着手）
- [ ] 8.1 (P) VisualConfig レイヤー構造エディターの実装
  - `character-visual-editor.ts` に体型・目・髪・服の色・肌色のレイヤーを選択できるUIを実装する
  - 選択内容をリアルタイムでSVGプレビューに反映する
  - 設定を `VisualConfig`（mode: 'template'）として `CharacterSchema.visualConfig` に格納し、`character-editor.ts` から呼び出せることを確認する
  - _Requirements: 6.1, 6.2_
  - _Boundary: character-visual-editor.ts_

- [ ] 8.2 (P) 自作画像アップロードと著作権同意フローの実装
  - PNG/SVGファイルのアップロード受け付け処理を実装する
  - アップロード前に著作権注意文（「既存のアニメ・ゲーム・商標キャラクターに似せた画像のアップロードは著作権侵害になる場合があります」）を表示し、同意確認ダイアログを実装する
  - ユーザーが同意した場合のみアップロードを進め、`VisualConfig`（mode: 'upload', uploadedImagePath）として格納する
  - 同意拒否時はアップロードをキャンセルし、既存のビジュアル設定を維持することを確認する
  - _Requirements: 6.3, 6.4, 6.5_
  - _Boundary: character-visual-editor.ts_

## Implementation Notes
- 1.1: `src/character-validator.ts` に CharacterSchema/VisualConfig 型・`AI_DISCLOSURE` 定数・`validate()`（aiDisclosure 強制付与=上書き不可、name/tone 非空チェック）を実装。フロントのテスト基盤として **vitest** を導入（`pnpm test`=`vitest run`）。
- 2.1(調整): design の `save_character(schema_json)` は storage-manager 既存の `save_character(name,data)` コマンドを**再利用**して満たす（コマンド名衝突回避。フロントは name=id, data=schema で呼ぶ）。新規追加は `delete_character`（ローカルのみ・サニタイズ・冪等。GDrive削除は storage 要件4.2 で禁止のため行わない）と `load_characters`（全キャラの**フルJSON**を Vec で返す。破損ファイルは起動時復元のためスキップ）。両コマンドを lib.rs に登録。`list_characters`(名前一覧)とは別物。
