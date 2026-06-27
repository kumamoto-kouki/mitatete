# Requirements Document

## Project Description (Input)

**誰の課題か：** Mitatete を使うデスクトップユーザー（自分で選んだキャラクターと日々対話し、その関係性の記録を残したい人）。

**現状：** character-layer により `CharacterSchema`（キャラクター定義・原則値）が確立され、storage-manager によりローカル常時保存と承認時の Google ドライブ同期が整備済み。原則 9 の強度導出式 `calcDiaryIntensity`（`src/principles.ts`）も実装済みだが、`src/diary.ts` はスタブのままで、対話履歴から AI 視点の観察日記を生成する手段が存在しない。

**変えること：** diary-engine（原則 9「観察を記述する、評価しない」）を実装し、当日の対話履歴から AI 一人称の観察日記を生成・表示・保存できるようにする。日記の ON/OFF と詳細度（強度）は原則値から自動導出し、システムが評価・断定を行わない観察記録としてユーザーが自分で気づきを得られる体験を提供する。

### 主要な振る舞い（初期 spec.md より引き継ぎ）

- 原則 9 が ON のとき、当日の対話履歴を収集して AI 視点の観察日記を生成する。
- 強度は `calcDiaryIntensity`（余白×0.4＋距離感×0.3＋多様×0.2＋行動×0.1）で原則値から自動導出し、強度に応じて詳細度（キーワード〜詳細観察）を変える。
- 生成文は評価・断定・感情の模倣を含まない観察文のみで構成し、「今日、{キャラクター名}として対話を記録する。」で始め、AI が生成した記録であることを明示する。
- 原則 9 が OFF のときは日記を生成せず、対話履歴を日記用途で使用しない。
- 生成した日記は画面に表示し、storage-manager 経由で保存する。Google ドライブ未承認時はローカル保存（常時利用可）にとどまり、クラウド同期は行わない。

### 日記の詳細度（強度導出値による）

| 強度（導出値） | 日記の詳細度              |
| -------------- | ------------------------- |
| 1〜2           | キーワードのみ（3〜5 語） |
| 3              | 短文観察（2〜3 文）       |
| 4              | 段落観察（5〜8 文）       |
| 5              | 詳細観察（10 文以上）     |

### 依存

- character-layer（`CharacterSchema`・原則値・キャラクター名を受け取る）— 実装完了済み。
- storage-manager（対話履歴の読み込み `read_history`・日記の保存 `save_diary`）— 実装完了済み。

> 詳細な初期メモは [`spec.md`](spec.md) を参照。本ファイルが正式な要件定義の正本となる。

## Introduction

Diary Engine は原則 9「観察を記述する、評価しない」の実装である。原則 9 が ON のとき、当日の対話履歴を収集し、アクティブな `CharacterSchema` のキャラクター名を冠した AI 一人称の観察日記を生成して画面に表示し、storage-manager 経由で保存する。日記の ON/OFF と詳細度は原則値から `calcDiaryIntensity` で自動導出され、システムが評価・断定・感情の模倣を行わない観察記録として、ユーザーが自分で気づきを得られる体験を提供する。日記の有無や強度はユーザーの原則設定（明示操作）のみに由来し、システムが自動でキャラクターや原則を変更・選択することはない（ユーザー自律性の尊重）。AI が生成した記録であることはすべての出力で明示する（原則 8）。

## Boundary Context

- **In scope**: 原則 9 の ON/OFF 状態に応じた日記生成の実行・抑止、原則値からの強度の自動導出（`calcDiaryIntensity` と整合）、当日の対話履歴の収集・整形、強度に応じた詳細度での AI 一人称観察日記の生成、観察のみ・AI 明示・固定書き出しという書き方制約の適用、生成日記の画面表示、storage-manager への日記保存の依頼。
- **Out of scope**: 対話履歴・日記ファイルの実 I/O（storage-manager が担う）、Google ドライブの承認状態管理と OAuth（storage-manager が担う）、キャラクター定義・原則値・原則 9 の ON/OFF を編集する UI（character-layer・原則エンジンが担う）、モデル API の呼び出し基盤（model-router が担う）。
- **Adjacent expectations**: character-layer から現在アクティブな `CharacterSchema`（キャラクター名）と原則値を受け取れること。storage-manager が当日の対話履歴を提供（`read_history`）し、生成した日記を永続化（`save_diary`）でき、Google ドライブ未承認時はローカルに保存し承認時のみ同期すること。

## Requirements

### Requirement 1: 原則 9 の ON/OFF に応じた日記生成の制御

**Objective:** Mitatete ユーザーとして、原則 9 の ON/OFF 設定どおりに日記生成が制御されてほしい。これにより、観察日記を残すかどうかを自分の意思で決められる。

#### Acceptance Criteria

1. While 原則 9 が ON である, when 日記生成がトリガーされる, the Diary Engine shall 当日の対話履歴を収集して観察日記を生成する。
2. If 原則 9 が OFF である, then the Diary Engine shall 日記を生成せず、対話履歴を日記用途で使用しない。
3. The Diary Engine shall 原則 9 の ON/OFF をユーザーの明示操作にのみ従わせ、システムの判断で日記生成を自動的に有効化・無効化しない。

### Requirement 2: 強度の自動導出と詳細度の決定

**Objective:** Mitatete ユーザーとして、原則設定に応じた粒度で日記が書かれてほしい。これにより、自分の原則の重み付けが日記の詳しさに反映される。

#### Acceptance Criteria

1. When 日記を生成する, the Diary Engine shall アクティブな原則値から `calcDiaryIntensity`（余白×0.4＋距離感×0.3＋多様×0.2＋行動×0.1）と整合する強度を自動導出する。
2. When 強度が導出される, the Diary Engine shall その強度に応じた詳細度（強度 1〜2＝キーワードのみ、3＝短文観察、4＝段落観察、5＝詳細観察）で日記を生成する。
3. The Diary Engine shall 強度の導出をアクティブな原則値にのみ基づかせ、利用状況などに基づく自動調整を行わない。

### Requirement 3: 当日の対話履歴の収集

**Objective:** Mitatete ユーザーとして、その日の対話だけが日記の対象になってほしい。これにより、日記が当日の記録として正確に保たれる。

#### Acceptance Criteria

1. When 日記を生成する, the Diary Engine shall storage-manager に当日の対話履歴の読み込み（`read_history`）を依頼し、当日分の履歴を収集する。
2. If 当日の対話履歴が存在しない, then the Diary Engine shall 日記を生成せず、対象履歴がない旨をユーザーに提示する。
3. The Diary Engine shall 収集する履歴を当日分に限定し、過去日の履歴を日記の対象に含めない。

### Requirement 4: 観察日記の生成と書き方制約

**Objective:** Mitatete ユーザーとして、日記が評価ではなく観察として書かれてほしい。これにより、結論を押し付けられず自分で気づける。

#### Acceptance Criteria

1. When 観察日記を生成する, the Diary Engine shall 評価・断定・感情の模倣を含まない観察文のみで本文を構成する。
2. The Diary Engine shall 日記の書き出しを「今日、{キャラクター名}として対話を記録する。」で開始する。
3. The Diary Engine shall 結論を本文に書かず、読み手が自分で気づける観察記述にとどめる。
4. The Diary Engine shall 生成した日記に AI（Mitatete）が生成した観察記録であることの明示を含める。

### Requirement 5: 日記の表示と保存

**Objective:** Mitatete ユーザーとして、生成された日記を読めて、承認状態に応じて適切に保存してほしい。これにより、未承認でも記録を失わず、承認時は同期もできる。

#### Acceptance Criteria

1. When 日記が生成される, the Diary Engine shall その日記をユーザーが読める形で画面に表示する。
2. When 日記が生成される, the Diary Engine shall storage-manager に当日の日記の保存（`save_diary`）を依頼する。
3. Where Google ドライブが未承認である, the Diary Engine shall ローカル保存のみを依頼し、クラウド同期を要求しない。
4. The Diary Engine shall 表示・保存の前後を通じて、生成した観察文の内容を改変しない。

### Requirement 6: 生成エラーの扱い

**Objective:** Mitatete ユーザーとして、日記生成に失敗したときに状況が分かり、不完全な記録が残らないでほしい。これにより、安心して再試行できる。

#### Acceptance Criteria

1. If 日記の生成に失敗する, then the Diary Engine shall 失敗を示すメッセージをユーザーに表示する。
2. If 日記の生成に失敗する, then the Diary Engine shall 不完全な日記を保存するよう storage-manager に依頼しない。
