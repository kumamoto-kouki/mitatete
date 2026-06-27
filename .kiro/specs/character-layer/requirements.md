# 要件定義書

## はじめに

Mitatete はAIモデルを「見立て」によって擬人化するTauriデスクトップアプリである。キャラクター層（character-layer）は、ユーザーがプリセットキャラクターを選択するか、カスタムキャラクターを作成することで、AIに名前・ビジュアル・口調を付与するコンポーネントである。

**誰が問題を抱えているか：** デスクトップAIアシスタントのユーザーは、AIを「道具」としてしか認識できず、心理的距離が縮まらない。AIとの対話が機械的になりやすい。

**現状の問題：** AIモデルはデフォルトで無名・無個性であり、ユーザーが「誰か」として認識できる存在になっていない。また、キャラクター設定が存在しても、原則エンジンやチャットUIへ一貫した形式で渡されない。

**何を変えるか：** プリセット／カスタムのキャラクター設定を共通の `CharacterSchema` に変換し、原則エンジン・チャットUI・キャラクターウィンドウへ一元的に供給する。原則8（AIであることを明示する文）は固定フィールドとして必ず含める。

## スコープ（境界コンテキスト）

- **スコープ内：** プリセットキャラクターの読み込みと選択、カスタムキャラクターの作成・編集・削除、`CharacterSchema` の生成と管理、キャラクター設定のローカルファイルシステムへの永続化（Rustバックエンド経由）、キャラクター切り替え時の原則エンジンへの初期値反映、フェーズ2のビジュアルエディター（レイヤー構造・著作権同意）
- **スコープ外：** 原則エンジンの7軸調整ロジック（model-router spec が担う）、チャットUI（別spec）、日記エンジン（diary-engine spec）、Googleドライブ同期（storage-manager spec）、AIモデルへのAPI呼び出し
- **隣接する期待：** `model-router` は本specが生成する `CharacterSchema` を入力として受け取り、プロンプトを構築する。`storage-manager` はキャラクター定義ファイルの物理的な保存・読み込みI/Oを担う

## 要件

### 要件 1：プリセットキャラクターの選択

**目的：** ユーザーとして、用意されたプリセットキャラクターを選択することで、すぐにキャラクターを設定できる。これにより、設定の手間なく対話を開始できる。

#### 受け入れ基準

1. When ユーザーがキャラクター選択画面を開く, the character-layer shall プリセットキャラクター一覧を表示する
2. When ユーザーがプリセットキャラクターを選択する, the character-layer shall そのキャラクターの名前・ビジュアル・口調・原則初期値を読み込み、`CharacterSchema` を生成する
3. When プリセットキャラクターの `CharacterSchema` が生成される, the character-layer shall `aiDisclosure` フィールド（「私はAIアシスタントです」系の文言）を固定値として必ず含める
4. The character-layer shall プリセットキャラクター定義を `src/assets/presets/*.json` から読み込む
5. If プリセット定義ファイルが存在しない場合, the character-layer shall エラーをユーザーに通知し、キャラクターなしの状態で起動を継続する

### 要件 2：カスタムキャラクターの作成

**目的：** ユーザーとして、独自の名前・口調・ビジュアルを設定したカスタムキャラクターを作成できる。これにより、自分だけのキャラクターで対話を行える。

#### 受け入れ基準

1. When ユーザーがカスタムキャラクター作成を開始する, the character-layer shall 名前（テキスト）・口調（テキスト）・ビジュアル（画像アップロード）の入力フォームを表示する
2. When ユーザーがカスタムキャラクターの必須項目（名前・口調）を入力して保存する, the character-layer shall `CharacterSchema` を生成してローカルファイル（`~/.mitatete/characters/`）に保存する
3. If カスタムキャラクターのビジュアルが未設定の場合, the character-layer shall デフォルトアバターを `visual` フィールドに使用する
4. When カスタムキャラクターの `CharacterSchema` が生成される, the character-layer shall `aiDisclosure` フィールドを固定値として必ず含める（ユーザーによる変更不可）
5. The character-layer shall カスタムキャラクターの保存・読み込みをRustバックエンドのTauriコマンド経由で行い、フロントエンドからファイルシステムに直接アクセスしない

### 要件 3：原則8の不変性

**目的：** システムとして、AIであることの明示を全キャラクターで維持する。これにより、ユーザーが常にAIと対話していることを認識できる。

#### 受け入れ基準

1. The character-layer shall すべての `CharacterSchema`（プリセット・カスタム問わず）に `aiDisclosure` フィールドを必ず含める
2. The character-layer shall `aiDisclosure` フィールドをUIから編集不可（読み取り専用）として扱う
3. The character-layer shall `CharacterSchema` を原則エンジンへ渡す際、`aiDisclosure` が空文字または未定義でないことを検証する

### 要件 4：キャラクターの切り替え

**目的：** ユーザーとして、使用するキャラクターをいつでも切り替えられる。これにより、状況に応じたキャラクターで対話できる。

#### 受け入れ基準

1. When ユーザーが別のキャラクターに切り替える, the character-layer shall 選択したキャラクターの `CharacterSchema` を読み込み、原則エンジンの設定値をそのキャラクターの `principleDefaults` で更新する
2. The character-layer shall キャラクターの切り替えを常にユーザーの明示的な操作によってのみ行う（システムによる自動切り替え・自動選択を行わない）
3. The character-layer shall 対話中にシステムがキャラクター設定をバックグラウンドで変更しない
4. When キャラクターが切り替えられる, the character-layer shall キャラクターウィンドウ（`character.html`）の表示を新しいキャラクターのビジュアルに更新する

### 要件 5：キャラクター設定の永続化

**目的：** ユーザーとして、キャラクター設定をアプリ再起動後も維持できる。これにより、毎回設定をやり直す手間がなくなる。

#### 受け入れ基準

1. When ユーザーがキャラクター設定を保存する, the character-layer shall Rustバックエンド経由でローカルファイルシステム（`~/.mitatete/characters/`）に永続化する
2. When アプリが起動する, the character-layer shall 最後に使用したキャラクターをローカルファイルから読み込んで復元する
3. If ローカルファイルの読み込みに失敗した場合, the character-layer shall デフォルトキャラクター（またはプリセット第一候補）を使用して起動する
4. The character-layer shall 保存処理をGoogleドライブの承認状態に依存させない（ローカル保存は常時動作する）

### 要件 6：ビジュアルエディター（フェーズ2）

**目的：** ユーザーとして、レイヤー構造でキャラクターの外見をカスタマイズできる。これにより、テンプレートをベースにオリジナルキャラクターを作れる。

#### 受け入れ基準

1. Where ビジュアルエディター機能が有効の場合, the character-layer shall 体型・目の形・髪・服の色・肌色のレイヤーをリアルタイムでプレビューしながら編集できる画面を表示する
2. Where ビジュアルエディター機能が有効の場合, the character-layer shall `VisualConfig`（mode: 'template'）として設定を `CharacterSchema` に格納する
3. When ユーザーが自作画像をアップロードする, the character-layer shall PNG/SVGファイルを受け付け、著作権に関する注意文と同意確認を表示してから処理を進める
4. If ユーザーが著作権同意を拒否した場合, the character-layer shall 画像アップロードをキャンセルし、既存のビジュアル設定を維持する
5. Where ビジュアルエディター機能が有効の場合, the character-layer shall `VisualConfig`（mode: 'upload'）の場合、アップロード済み画像をローカルファイルパス（`uploadedImagePath`）で参照する
