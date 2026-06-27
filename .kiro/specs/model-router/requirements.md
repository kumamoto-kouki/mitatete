# Requirements Document

## Project Description (Input)

**誰の課題か：** Mitatete を使うデスクトップユーザー（自分で選んだキャラクターと対話したい人）。

**現状：** character-layer により `CharacterSchema`（キャラクター定義）と 7 原則の強度値は確立済みだが、それらを実際のモデル API へ送信して対話する手段がまだ存在しない。チャット UI（`main.ts`）は入力受付のスタブにとどまる。

**変えること：** model-router を実装し、Claude・GPT・Gemini を切り替えながら、`CharacterSchema` と原則値を組み込んだシステムプロンプトを構築してモデル API へ送信し、応答をチャット UI に表示できるようにする（マイルストーン M3「モデルと実対話」の前提）。

### 主要な振る舞い（初期 spec.md より引き継ぎ）

- モデル切替は次のリクエストから選択モデルの API を使用する。
- システムプロンプトに `CharacterSchema` と原則値を組み込み、原則 8 の `aiDisclosure` を必ず含める。
- API キーは Rust バックエンド（`key_manager.rs` 相当）で OS セキュアストレージに保存し、フロントエンド・ネットワークへ露出しない（Tauri コマンド経由でのみアクセス）。
- API キー未設定のモデルが選択されたら設定画面へ誘導し、リクエストを送信しない。
- API レスポンスがエラーの場合はチャット UI にエラーを表示し、対話履歴には記録しない。

### サポートモデル

| モデル              | エンドポイント                                         | 認証                  |
| ------------------- | ------------------------------------------------------ | --------------------- |
| Claude（Anthropic） | `https://api.anthropic.com/v1/messages`                | API キー（x-api-key） |
| GPT（OpenAI）       | `https://api.openai.com/v1/chat/completions`           | API キー（Bearer）    |
| Gemini（Google）    | `https://generativelanguage.googleapis.com/v1beta/...` | API キー              |

### 依存

- character-layer（`CharacterSchema` と原則値を受け取る）— 実装完了済み。
- storage-manager（対話履歴の保存）— 実装完了済み。

> 詳細な初期メモは [`spec.md`](spec.md) を参照。本ファイルが正式な要件定義の正本となる。

## Introduction

Model Router は、ユーザーが選んだ AI モデル（Claude / GPT / Gemini）に対し、character-layer が確立した `CharacterSchema` と 7 原則の強度値を組み込んだシステムプロンプトを構築して送信し、応答をチャット UI に返すコンポーネントである。原則 8（`aiDisclosure`）を必ずプロンプトへ含め、API キーはユーザー本人のものをセキュアに保持してフロントエンド・外部ネットワークへ露出しない。モデル選択はユーザーの明示操作のみで行い、システムが自動で選択・切替を行わない（ユーザー自律性の尊重）。マイルストーン M3「モデルと実対話」を実現する。

## Boundary Context

- **In scope**: モデルの選択・切替、システムプロンプト構築（キャラクター定義＋原則値＋`aiDisclosure`）、API キーの設定受付と保護・未設定時の誘導、選択モデル API への送信と応答表示、API エラー時の表示と履歴非記録、応答成功時の対話履歴記録の依頼。
- **Out of scope**: API キーの物理的なセキュア保存実装（バックエンドのセキュアストレージが担う）、対話履歴ファイルの実 I/O（storage-manager が担う）、キャラクター定義・原則値の編集 UI（character-layer が担う）、Google ドライブ同期。
- **Adjacent expectations**: character-layer から現在アクティブな `CharacterSchema` と原則値を受け取れること。storage-manager が対話履歴を永続化できること。バックエンドのセキュアストレージが API キーを保管し、フロントエンドへ平文を返さないこと。

## Requirements

### Requirement 1: モデルの選択と切替

**Objective:** Mitatete ユーザーとして、使用する AI モデルを自分で選んで切り替えたい。これにより、目的に応じたモデルで対話できる。

#### Acceptance Criteria

1. When ユーザーが対応モデル（Claude / GPT / Gemini）を選択する, the Model Router shall そのモデルをアクティブモデルとして保持する。
2. When ユーザーがモデルを切り替える, the Model Router shall 切替後の次のリクエストから選択されたモデルの API を使用する。
3. The Model Router shall モデルの選択・切替をユーザーの明示操作によってのみ行い、利用状況などに基づく自動選択・自動切替を行わない。
4. While 送信中のリクエストが処理中である, when ユーザーがモデルを切り替える, the Model Router shall 処理中のリクエストには影響させず、次回リクエストから新モデルを適用する。

### Requirement 2: システムプロンプトの構築

**Objective:** Mitatete ユーザーとして、選んだキャラクターと原則設定が応答に反映されてほしい。これにより、一貫した固有性のある対話ができる。

#### Acceptance Criteria

1. When リクエストを送信する, the Model Router shall アクティブな `CharacterSchema`（名前・口調）と原則値をシステムプロンプトに組み込む。
2. The Model Router shall すべてのリクエストのシステムプロンプトに原則 8 の `aiDisclosure`（AI であることの明示）を必ず含める。
3. When システムプロンプトを構築する, the Model Router shall 原則値の優先度・強度に応じた行動ガイドラインを反映する。
4. The Model Router shall ユーザーの入力テキストをシステムプロンプトとは区別してモデルへ渡す。

### Requirement 3: API キーの設定と保護

**Objective:** Mitatete ユーザーとして、自分の API キーを安全に設定したい。これにより、キーが漏洩する不安なく各モデルを利用できる。

#### Acceptance Criteria

1. When ユーザーが設定画面で API キーを入力して保存する, the Model Router shall そのキーをセキュアストレージへ保存するよう依頼する。
2. The Model Router shall API キーを、対象モデルの公式 API 送信先以外のネットワーク宛先へ送信しない。
3. The Model Router shall API キーの平文をフロントエンドの表示・ログ・対話履歴へ出力しない。
4. If API キーが未設定のモデルが選択された状態でユーザーがメッセージを送信する, then the Model Router shall API キー設定画面へ誘導し、モデル API へのリクエストを送信しない。

### Requirement 4: リクエスト送信と応答表示

**Objective:** Mitatete ユーザーとして、入力したメッセージへのモデル応答を受け取りたい。これにより、キャラクターと実際に対話できる。

#### Acceptance Criteria

1. When ユーザーがメッセージを送信し、選択モデルに有効な API キーがある, the Model Router shall 構築したプロンプトを選択モデルの API へ送信する。
2. When モデルから正常な応答を受信する, the Model Router shall その応答をチャット UI に表示する。
3. Where ストリーミング応答が利用可能な場合, the Model Router shall 応答を逐次的にチャット UI へ表示する。
4. While モデルの応答を待機している, the Model Router shall 応答待ちであることがユーザーに分かる状態を提示する。

### Requirement 5: エラーハンドリング

**Objective:** Mitatete ユーザーとして、API エラー時に状況が分かり、失敗が履歴を汚さないでほしい。これにより、安心して再試行できる。

#### Acceptance Criteria

1. If モデル API がエラー応答を返す, then the Model Router shall エラーの内容をチャット UI に表示する。
2. If モデル API がエラー応答を返す, then the Model Router shall その失敗した送受信を対話履歴に記録しない。
3. If ネットワーク到達不能などでリクエストが失敗する, then the Model Router shall 失敗を示すメッセージを表示し、リクエストを成功扱いにしない。

### Requirement 6: 対話履歴の記録

**Objective:** Mitatete ユーザーとして、成立した対話だけが履歴に残ってほしい。これにより、履歴が正確に保たれる。

#### Acceptance Criteria

1. When モデル応答が正常に受信・表示される, the Model Router shall ユーザー入力とモデル応答の対を対話履歴へ記録するよう storage-manager に依頼する。
2. The Model Router shall 応答が得られなかった（エラー・キャンセル・キー未設定）リクエストを対話履歴へ記録しない。
