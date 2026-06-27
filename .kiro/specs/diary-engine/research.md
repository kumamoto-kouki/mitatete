# Research Log — diary-engine

## ディスカバリ範囲

既存コードベースへの拡張（Light Discovery）。スタブ `src/diary.ts` を実装し、character-layer・storage-manager・model-router の既存シームを統合する。新規 Rust バックエンドは原則不要（既存 Tauri コマンドの再利用）。

## 既存シーム（再利用）

- **強度導出**: `src/principles.ts` の `calcDiaryIntensity(principles)`（余白×0.4＋距離感×0.3＋多様×0.2＋行動×0.1）を**そのまま再利用**（要件2.1）。
- **対話履歴**: storage-manager の Tauri コマンド `read_history(date)`（YYYY-MM-DD、当日分を返す）。当日履歴の収集に使う（要件3.1）。
- **日記保存**: storage-manager の `save_diary(date, content)`。**Google ドライブ未承認時はローカルのみ・承認時のみ同期**を storage 側が自動処理する（要件5.2, 5.3 は storage が担保）。diary-engine は呼ぶだけ。
- **アクティブキャラクター・原則9 ON/OFF**: character-layer の `character-store.getActive()` から `CharacterSchema`（`name`・`principleDefaults`・`diaryEnabled`＝原則9 ON/OFF）を取得（要件1, 2, 4.2）。
- **日付**: 当日 `YYYY-MM-DD` はフロントで生成。

## 重要な設計判断（要 synthesis / Open Question）

### 観察日記本文の生成機構

要件4（観察のみ・評価/断定/感情模倣を含まない・強度に応じた詳細度 キーワード〜10文以上）は **LLM 生成**を前提とする。Mitatete でモデル API を呼ぶ唯一の基盤は **model-router**。

- **判断**: diary-engine は本文生成を **model-router に委譲**する。diary-engine は「日記用システムプロンプト（観察制約＋固定書き出し＋強度→詳細度＋AI明示）」を構築し、当日履歴をメッセージとして渡してモデル応答（観察文）を得る。
- **影響（steering グラフの拡張）**: structure.md の依存グラフは DE→CL・SM のみ。本判断で **DE→MR** の依存辺が増える。model-router 側に「呼び出し元が system プロンプトを供給できる汎用生成エントリ」が必要（現状 `send_message` はキャラクター用 system を内部構築するため、diary 用 system を渡せない）。
  - **推奨**: model-router を「`generate(system_prompt, messages) -> text`（汎用）」＋「`send_message`（キャラ用 system を構築して generate を呼ぶ薄いラッパ）」に整理する。これは model-router の再検証トリガー。
- **代替案（不採用）**: 履歴から決定的にテンプレ生成（LLM 不使用）。強度4〜5（段落〜10文以上の観察）を満たせず、観察文の自然さも劣るため不採用。

> Open Question（ユーザー確認）: ①DE→MR 依存を許容し model-router に汎用生成エントリを設けるか、②diary-engine 実装は model-router 完成後に着手するか（依存順）。

### 生成トリガー

要件1.1「日記生成がトリガーされる」の契機は未指定。ユーザー自律性（自動生成しない＝要件1.3）に従い、**ユーザー操作（「今日の日記を生成」ボタン）**を MVP のトリガーとする。アプリ終了時の自動生成等は将来検討。

## リスク

- **model-router 未完成**: 本 spec の本文生成は model-router の汎用生成エントリに依存。順序として model-router 実装完了後に diary-engine 実装着手が安全。
- **強度→詳細度の一貫性**: `calcDiaryIntensity` の値域（おおよそ1〜5の加重和、四捨五入で1〜5バケット）を詳細度マッピングに変換する境界を design で固定する。
- **観察制約の検証**: 「評価・断定を含まない」はプロンプト制約であり、出力の完全保証は不可。固定書き出し・AI明示は構造的に検証可能（テスト対象）。
