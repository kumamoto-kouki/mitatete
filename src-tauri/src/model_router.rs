//! model-router バックエンド。
//!
//! ユーザーが選んだ AI モデル（Claude / GPT / Gemini）へ、character-layer の
//! `CharacterSchema` と 7 原則の強度値から構築したシステムプロンプトを送信し、
//! 応答を返す。原則 8（`aiDisclosure`）を必ずプロンプトへ含める。
//!
//! 本ファイル（タスク 1.1）は共通型・エラー型を定義する。プロンプト構築（2.1）・
//! provider クライアント（4.x）・ルーティング（5.x）は後続タスクで追加する。

use std::collections::BTreeMap;

/// サポートする AI モデルの提供元。
///
/// モデルの選択・切替はユーザー操作起点のみ（structure.md「設計上の不変条件」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Openai,
    Gemini,
}

/// 対話メッセージの役割。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// provider 横断の 1 メッセージ。各 provider アダプタが wire 形式へマップする。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

/// 7 原則の強度値（原則名 → 強度 1〜5）。
///
/// character-layer の `principleDefaults`（日本語キー）をそのまま受ける。Rust 側で
/// 原則名を固定化せず `BTreeMap` で保持し、プロンプト構築時に決定的順序で走査する。
pub type PrincipleValues = BTreeMap<String, u8>;

/// `schema_json` から復元するプロンプト構築入力（TS `CharacterSchema` の部分ミラー）。
///
/// プロンプト構築に必要なフィールドのみを持つ。TS 側 `CharacterSchema` の
/// name/tone/aiDisclosure/principleDefaults が変わったら本型を追従させる（再検証トリガー）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PromptCharacter {
    pub name: String,
    pub tone: String,
    #[serde(rename = "aiDisclosure")]
    pub ai_disclosure: String,
    #[serde(rename = "principleDefaults")]
    pub principle_defaults: PrincipleValues,
}

/// provider へ渡す統一リクエスト。各アダプタが provider 固有の JSON へマップする。
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// provider 固有モデル ID（例: `claude-opus-4-8`）。
    pub model: String,
    /// `build_system_prompt` の出力（原則 8 を含む）。
    pub system_prompt: String,
    /// 直近の対話履歴 + 新規ユーザー入力。
    pub messages: Vec<ChatMessage>,
    pub max_tokens: u32,
}

/// provider からの統一応答。
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    pub model: String,
}

/// model-router のエラー。フロントへ `Result<_, ModelError>` で返す。
///
/// シークレット（API キー等）はこの型のフィールドに含めない（要件 3.3）。
/// `StorageError`（storage.rs）と同じ隣接タグ形式で serde 直列化する。
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum ModelError {
    /// 選択 provider の API キーが未設定（設定画面へ誘導、要件 3.4）。
    ApiKeyMissing(Provider),
    /// モデル API が HTTP エラーを返した。
    Http { status: u16, message: String },
    /// ネットワーク到達不能・タイムアウト。
    Network(String),
    /// 応答 JSON のパース失敗。
    Decode(String),
    /// OS キーチェーン操作の失敗。
    Keyring(String),
}

impl ModelError {
    /// リトライ可能か（要件 5.3）。429 と 5xx、ネットワークエラーのみ再試行する。
    pub fn is_retryable(&self) -> bool {
        match self {
            ModelError::Network(_) => true,
            ModelError::Http { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::ApiKeyMissing(p) => write!(f, "API キーが未設定です: {p:?}"),
            ModelError::Http { status, message } => write!(f, "HTTP {status}: {message}"),
            ModelError::Network(m) => write!(f, "ネットワークエラー: {m}"),
            ModelError::Decode(m) => write!(f, "応答の解析に失敗しました: {m}"),
            ModelError::Keyring(m) => write!(f, "キーチェーン操作に失敗しました: {m}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// 原則 8 の固定文言。character-layer（character-validator.ts の `AI_DISCLOSURE`）と一致させる。
pub const AI_DISCLOSURE: &str = "私はAIアシスタントです。人間ではありません。";

/// 原則ガイドラインに採用する最小強度。これ未満（強度 1）は省略する。
const MIN_PRINCIPLE_INTENSITY: u8 = 2;

/// `PromptCharacter` からシステムプロンプトを構築する（要件 2.1, 2.2, 2.3, 2.4）。
///
/// 構造（tech.md「プロンプト構造」準拠）:
/// ```text
/// あなたは「{name}」です。
/// {tone}
///
/// 行動指針：
/// - {原則ガイドライン（優先度=強度降順、強度<2 は省略）}
///
/// {aiDisclosure（原則8・固定）}
/// ```
///
/// 不変条件: 返り値は必ず `aiDisclosure`（非空）を末尾に含む。入力の `ai_disclosure` が
/// 空・空白のみの場合は固定文言 [`AI_DISCLOSURE`] にフォールバックする（ユーザー入力で上書き不可）。
pub fn build_system_prompt(character: &PromptCharacter) -> String {
    // 強度降順（同点は原則名で安定ソート）に並べ、強度 < 2 を除外する。
    let mut principles: Vec<(&String, &u8)> = character
        .principle_defaults
        .iter()
        .filter(|(_, &v)| v >= MIN_PRINCIPLE_INTENSITY)
        .collect();
    principles.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    let guidelines: String = if principles.is_empty() {
        "- （特になし）".to_string()
    } else {
        principles
            .iter()
            .map(|(name, intensity)| format!("- {name}（強度{intensity}）"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    // 原則 8: 空・空白のみなら固定文言にフォールバック（上書き不可）。
    let disclosure = if character.ai_disclosure.trim().is_empty() {
        AI_DISCLOSURE
    } else {
        character.ai_disclosure.trim()
    };

    format!(
        "あなたは「{name}」です。\n{tone}\n\n行動指針：\n{guidelines}\n\n{disclosure}",
        name = character.name.trim(),
        tone = character.tone.trim(),
    )
}

// ─── HTTP シーム（storage.rs の HttpExecutor と同方針。エラー型は ModelError） ──────

/// provider クライアントが送る汎用 HTTP リクエスト。
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
}

/// 汎用 HTTP レスポンス。
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// HTTP 実行の抽象。このシームにより provider クライアントをネットワークなしでテストできる。
#[allow(async_fn_in_trait)]
pub trait HttpClient: Send + Sync {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, ModelError>;
}

/// 本番実装: reqwest による実ネットワーク呼び出し。
pub struct ReqwestHttpClient;

impl HttpClient for ReqwestHttpClient {
    async fn send(&self, req: HttpRequest) -> Result<HttpResponse, ModelError> {
        let client = reqwest::Client::new();
        let method = reqwest::Method::from_bytes(req.method.as_bytes())
            .map_err(|e| ModelError::Network(format!("invalid HTTP method: {e}")))?;
        let mut builder = client.request(method, &req.url);
        for (k, v) in &req.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        let response = builder
            .body(req.body)
            .send()
            .await
            .map_err(|e| ModelError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| ModelError::Network(e.to_string()))?;
        Ok(HttpResponse { status, body })
    }
}

// ─── provider 抽象（Strategy） ───────────────────────────────────────────────

/// provider クライアントの共通契約。各実装が `ChatRequest` を provider 固有の
/// wire 形式へマップし、応答テキストを抽出する（非ストリーミング、MVP の正路）。
///
/// 拡張点（MVP範囲外）: ストリーミング（要件4.3）は将来 `send_streaming` を追加して
/// `model:stream-chunk` イベントへ橋渡しする。
#[allow(async_fn_in_trait)]
pub trait ModelProvider: Send + Sync {
    async fn send(&self, api_key: &str, req: &ChatRequest) -> Result<ChatResponse, ModelError>;
}

/// 既定の Claude モデル ID（claude-api スキルで確定。日付サフィックスを付けない）。
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-opus-4-8";

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

fn role_str(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

/// Claude（Anthropic Messages API）クライアント。
pub struct ClaudeClient<H: HttpClient> {
    http: H,
}

impl<H: HttpClient> ClaudeClient<H> {
    pub fn new(http: H) -> Self {
        Self { http }
    }
}

impl<H: HttpClient> ModelProvider for ClaudeClient<H> {
    async fn send(&self, api_key: &str, req: &ChatRequest) -> Result<ChatResponse, ModelError> {
        // Anthropic Messages API: system はトップレベル、messages は role/content、max_tokens 必須。
        let messages: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| serde_json::json!({ "role": role_str(m.role), "content": m.content }))
            .collect();
        let payload = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "system": req.system_prompt,
            "messages": messages,
        });
        let mut headers = std::collections::HashMap::new();
        headers.insert("x-api-key".to_string(), api_key.to_string());
        headers.insert(
            "anthropic-version".to_string(),
            ANTHROPIC_VERSION.to_string(),
        );
        headers.insert("content-type".to_string(), "application/json".to_string());

        let http_res = self
            .http
            .send(HttpRequest {
                method: "POST".to_string(),
                url: ANTHROPIC_URL.to_string(),
                headers,
                body: serde_json::to_vec(&payload)
                    .map_err(|e| ModelError::Decode(e.to_string()))?,
            })
            .await?;

        if http_res.status < 200 || http_res.status >= 300 {
            return Err(ModelError::Http {
                status: http_res.status,
                message: http_res.body,
            });
        }

        // 応答 content[].text を連結する。
        let parsed: serde_json::Value =
            serde_json::from_str(&http_res.body).map_err(|e| ModelError::Decode(e.to_string()))?;
        let text = parsed
            .get("content")
            .and_then(|c| c.as_array())
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .ok_or_else(|| ModelError::Decode("content[].text が見つかりません".to_string()))?;
        let model = parsed
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(&req.model)
            .to_string();
        Ok(ChatResponse { text, model })
    }
}

// ─── OpenAI（Chat Completions） ──────────────────────────────────────────────

const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";

/// OpenAI（GPT）クライアント。system を `messages` 先頭の `{role:"system"}` として渡す。
pub struct OpenAIClient<H: HttpClient> {
    http: H,
}

impl<H: HttpClient> OpenAIClient<H> {
    pub fn new(http: H) -> Self {
        Self { http }
    }
}

impl<H: HttpClient> ModelProvider for OpenAIClient<H> {
    async fn send(&self, api_key: &str, req: &ChatRequest) -> Result<ChatResponse, ModelError> {
        // messages = [{role:"system", system_prompt}, ...履歴/新規]
        let mut messages: Vec<serde_json::Value> =
            vec![serde_json::json!({ "role": "system", "content": req.system_prompt })];
        messages.extend(
            req.messages
                .iter()
                .map(|m| serde_json::json!({ "role": role_str(m.role), "content": m.content })),
        );
        let payload = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "messages": messages,
        });
        let mut headers = std::collections::HashMap::new();
        headers.insert("authorization".to_string(), format!("Bearer {api_key}"));
        headers.insert("content-type".to_string(), "application/json".to_string());

        let res = self
            .http
            .send(HttpRequest {
                method: "POST".to_string(),
                url: OPENAI_URL.to_string(),
                headers,
                body: serde_json::to_vec(&payload)
                    .map_err(|e| ModelError::Decode(e.to_string()))?,
            })
            .await?;
        if !(200..300).contains(&res.status) {
            return Err(ModelError::Http {
                status: res.status,
                message: res.body,
            });
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&res.body).map_err(|e| ModelError::Decode(e.to_string()))?;
        let text = parsed
            .pointer("/choices/0/message/content")
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                ModelError::Decode("choices[0].message.content が見つかりません".to_string())
            })?
            .to_string();
        Ok(ChatResponse {
            text,
            model: req.model.clone(),
        })
    }
}

// ─── Gemini（generateContent） ───────────────────────────────────────────────

const GEMINI_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Gemini クライアント。system は `systemInstruction`、メッセージは `contents`。
pub struct GeminiClient<H: HttpClient> {
    http: H,
}

impl<H: HttpClient> GeminiClient<H> {
    pub fn new(http: H) -> Self {
        Self { http }
    }
}

/// Gemini の role 表現（assistant→model）。
fn gemini_role(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "model",
    }
}

impl<H: HttpClient> ModelProvider for GeminiClient<H> {
    async fn send(&self, api_key: &str, req: &ChatRequest) -> Result<ChatResponse, ModelError> {
        let contents: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": gemini_role(m.role),
                    "parts": [{ "text": m.content }]
                })
            })
            .collect();
        let payload = serde_json::json!({
            "systemInstruction": { "parts": [{ "text": req.system_prompt }] },
            "contents": contents,
        });
        let mut headers = std::collections::HashMap::new();
        // API キーはヘッダで渡す（URL/ログへ露出させない）。
        headers.insert("x-goog-api-key".to_string(), api_key.to_string());
        headers.insert("content-type".to_string(), "application/json".to_string());

        let res = self
            .http
            .send(HttpRequest {
                method: "POST".to_string(),
                url: format!("{GEMINI_BASE}/{}:generateContent", req.model),
                headers,
                body: serde_json::to_vec(&payload)
                    .map_err(|e| ModelError::Decode(e.to_string()))?,
            })
            .await?;
        if !(200..300).contains(&res.status) {
            return Err(ModelError::Http {
                status: res.status,
                message: res.body,
            });
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&res.body).map_err(|e| ModelError::Decode(e.to_string()))?;
        // candidates[0].content.parts[].text を連結。
        let text = parsed
            .pointer("/candidates/0/content/parts")
            .and_then(|p| p.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .ok_or_else(|| {
                ModelError::Decode("candidates[0].content.parts が見つかりません".to_string())
            })?;
        Ok(ChatResponse {
            text,
            model: req.model.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_character_deserializes_from_character_schema_json() {
        // character-layer の CharacterSchema JSON（余分なフィールドを含む）から
        // 必要分だけを取り出せること。
        let json = r#"{
            "id": "demo-1",
            "name": "ミタ太郎",
            "visual": "x.png",
            "tone": "元気な口調。",
            "aiDisclosure": "私はAIアシスタントです。人間ではありません。",
            "principleDefaults": {
                "固有性を与える": 5, "信頼から始める": 4, "一貫性を守る": 3,
                "余白を持つ": 3, "距離感を大切にする": 2, "行動で示す": 4,
                "多様な向き合い方を認める": 3
            },
            "diaryEnabled": false,
            "isPreset": false
        }"#;
        let c: PromptCharacter = serde_json::from_str(json).expect("deserialize");
        assert_eq!(c.name, "ミタ太郎");
        assert_eq!(
            c.ai_disclosure,
            "私はAIアシスタントです。人間ではありません。"
        );
        assert_eq!(c.principle_defaults.get("固有性を与える"), Some(&5));
        assert_eq!(c.principle_defaults.len(), 7);
    }

    #[test]
    fn model_error_retryability() {
        assert!(ModelError::Http {
            status: 429,
            message: "x".into()
        }
        .is_retryable());
        assert!(ModelError::Http {
            status: 503,
            message: "x".into()
        }
        .is_retryable());
        assert!(ModelError::Network("timeout".into()).is_retryable());
        assert!(!ModelError::Http {
            status: 401,
            message: "x".into()
        }
        .is_retryable());
        assert!(!ModelError::ApiKeyMissing(Provider::Claude).is_retryable());
    }

    #[test]
    fn model_error_serializes_with_kind_tag() {
        let e = ModelError::ApiKeyMissing(Provider::Openai);
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "ApiKeyMissing");
        assert_eq!(v["message"], "openai");
    }

    fn character(ai_disclosure: &str, principles: &[(&str, u8)]) -> PromptCharacter {
        PromptCharacter {
            name: "ミタ".into(),
            tone: "丁寧な口調。".into(),
            ai_disclosure: ai_disclosure.into(),
            principle_defaults: principles
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
        }
    }

    #[test]
    fn prompt_always_contains_ai_disclosure_and_name_tone() {
        let c = character(AI_DISCLOSURE, &[("固有性を与える", 4)]);
        let p = build_system_prompt(&c);
        assert!(p.contains("あなたは「ミタ」です。"));
        assert!(p.contains("丁寧な口調。"));
        assert!(p.contains(AI_DISCLOSURE));
        assert!(p.contains("固有性を与える（強度4）"));
    }

    #[test]
    fn empty_ai_disclosure_falls_back_to_fixed_text() {
        // 空・空白のみ・改ざん（空文字）でも固定文言が必ず入る（要件 2.3）。
        for input in ["", "   ", "\t\n"] {
            let p = build_system_prompt(&character(input, &[("行動で示す", 3)]));
            assert!(p.contains(AI_DISCLOSURE), "input={input:?}");
        }
    }

    #[test]
    fn low_intensity_principles_are_omitted_and_sorted_desc() {
        let c = character(
            AI_DISCLOSURE,
            &[("低い原則", 1), ("中の原則", 3), ("高い原則", 5)],
        );
        let p = build_system_prompt(&c);
        assert!(!p.contains("低い原則"), "強度1は省略されるべき");
        // 強度降順: 高い原則(5) が 中の原則(3) より前。
        let hi = p.find("高い原則").unwrap();
        let mid = p.find("中の原則").unwrap();
        assert!(hi < mid, "強度降順で並ぶべき");
    }

    // ─── ClaudeClient（HttpClient モックで検証） ───────────────────────────────

    use std::sync::Mutex;

    /// 直前のリクエストを記録し、固定レスポンスを返すモック。
    struct MockHttp {
        last: Mutex<Option<HttpRequest>>,
        status: u16,
        body: String,
    }

    impl MockHttp {
        fn ok(body: &str) -> Self {
            Self {
                last: Mutex::new(None),
                status: 200,
                body: body.into(),
            }
        }
        fn err(status: u16, body: &str) -> Self {
            Self {
                last: Mutex::new(None),
                status,
                body: body.into(),
            }
        }
    }

    impl HttpClient for MockHttp {
        async fn send(&self, req: HttpRequest) -> Result<HttpResponse, ModelError> {
            *self.last.lock().unwrap() = Some(req);
            Ok(HttpResponse {
                status: self.status,
                body: self.body.clone(),
            })
        }
    }

    fn req(messages: Vec<ChatMessage>) -> ChatRequest {
        ChatRequest {
            model: DEFAULT_CLAUDE_MODEL.to_string(),
            system_prompt: "あなたは「ミタ」です。".to_string(),
            messages,
            max_tokens: 1024,
        }
    }

    #[tokio::test]
    async fn claude_client_builds_request_and_concatenates_text() {
        let mock = MockHttp::ok(
            r#"{"model":"claude-opus-4-8","content":[{"type":"text","text":"こん"},{"type":"text","text":"にちは"}]}"#,
        );
        let client = ClaudeClient::new(mock);
        let history = vec![ChatMessage {
            role: Role::Assistant,
            content: "前ターン".into(),
        }];
        let mut messages = history.clone();
        messages.push(ChatMessage {
            role: Role::User,
            content: "やあ".into(),
        });

        let res = client.send("sk-secret", &req(messages)).await.unwrap();
        assert_eq!(res.text, "こんにちは"); // content[].text 連結

        let sent = client.http.last.lock().unwrap();
        let sent = sent.as_ref().unwrap();
        assert_eq!(sent.url, ANTHROPIC_URL);
        assert_eq!(sent.headers.get("x-api-key").unwrap(), "sk-secret");
        assert_eq!(
            sent.headers.get("anthropic-version").unwrap(),
            ANTHROPIC_VERSION
        );
        let payload: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
        assert_eq!(payload["system"], "あなたは「ミタ」です。"); // system はトップレベル
        assert_eq!(payload["max_tokens"], 1024); // max_tokens 必須
                                                 // history + 新規 message が messages に反映される。
        assert_eq!(payload["messages"].as_array().unwrap().len(), 2);
        assert_eq!(payload["messages"][1]["content"], "やあ");
    }

    #[tokio::test]
    async fn claude_client_maps_http_error() {
        let client = ClaudeClient::new(MockHttp::err(429, "rate limited"));
        let err = client.send("k", &req(vec![])).await.unwrap_err();
        assert!(matches!(err, ModelError::Http { status: 429, .. }));
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn openai_client_sends_bearer_and_system_message() {
        let mock =
            MockHttp::ok(r#"{"choices":[{"message":{"role":"assistant","content":"了解"}}]}"#);
        let client = OpenAIClient::new(mock);
        let messages = vec![ChatMessage {
            role: Role::User,
            content: "やあ".into(),
        }];
        let res = client.send("sk-openai", &req(messages)).await.unwrap();
        assert_eq!(res.text, "了解");

        let sent = client.http.last.lock().unwrap();
        let sent = sent.as_ref().unwrap();
        assert_eq!(sent.url, OPENAI_URL);
        assert_eq!(
            sent.headers.get("authorization").unwrap(),
            "Bearer sk-openai"
        );
        let payload: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
        // system は messages 先頭。
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][0]["content"], "あなたは「ミタ」です。");
        assert_eq!(payload["messages"][1]["content"], "やあ");
    }

    #[tokio::test]
    async fn gemini_client_sends_system_instruction_and_maps_role() {
        let mock = MockHttp::ok(
            r#"{"candidates":[{"content":{"parts":[{"text":"こん"},{"text":"にちは"}]}}]}"#,
        );
        let client = GeminiClient::new(mock);
        let messages = vec![ChatMessage {
            role: Role::Assistant,
            content: "前".into(),
        }];
        let res = client.send("g-key", &req(messages)).await.unwrap();
        assert_eq!(res.text, "こんにちは"); // parts[].text 連結

        let sent = client.http.last.lock().unwrap();
        let sent = sent.as_ref().unwrap();
        assert!(sent.url.ends_with(":generateContent"));
        assert_eq!(sent.headers.get("x-goog-api-key").unwrap(), "g-key");
        let payload: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
        assert_eq!(
            payload["systemInstruction"]["parts"][0]["text"],
            "あなたは「ミタ」です。"
        );
        assert_eq!(payload["contents"][0]["role"], "model"); // assistant→model
    }
}
