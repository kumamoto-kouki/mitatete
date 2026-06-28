//! API キーの秘匿保存。
//!
//! ユーザー本人の API キー（Claude / OpenAI / Gemini）を OS キーチェーン（keyring）へ
//! 保存し、Rust の provider クライアントからのみ参照する。フロントエンド・ネットワーク・
//! 対話履歴へ平文を露出しない（要件 3.2, 3.3）。
//!
//! storage.rs の `TokenStore`/`KeyringTokenStore` と同じシーム方針: `KeyStore` trait で
//! 抽象化し、本番は keyring、テストはモックで検証する。OAuth トークン
//! （service=`mitatete-oauth`）とは別 service 名（`mitatete-apikeys`）で衝突を避ける。

use crate::model_router::{ModelError, Provider};

/// provider に対応する keyring のユーザー名（エントリ識別子）。
fn provider_username(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude => "claude",
        Provider::Openai => "openai",
        Provider::Gemini => "gemini",
    }
}

/// 照会対象の全 provider。
pub const ALL_PROVIDERS: [Provider; 3] = [Provider::Claude, Provider::Openai, Provider::Gemini];

/// API キー有無の照会結果（**平文キーを含まない**、要件 3.3）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiKeyStatus {
    pub provider: Provider,
    pub has_key: bool,
}

/// API キー保存の抽象（テスト容易性のためのシーム）。
pub trait KeyStore: Send + Sync {
    /// API キーを保存する（同一 provider は上書き）。
    fn set(&self, provider: Provider, key: &str) -> Result<(), ModelError>;
    /// API キーを取得する。Rust の provider クライアントからのみ呼ぶこと（フロントへ返さない）。
    fn get(&self, provider: Provider) -> Result<Option<String>, ModelError>;
    /// API キーの有無を返す。
    fn has(&self, provider: Provider) -> bool {
        matches!(self.get(provider), Ok(Some(_)))
    }
    /// 全 provider の有無一覧（平文を含まない）。
    fn status_all(&self) -> Vec<ApiKeyStatus> {
        ALL_PROVIDERS
            .iter()
            .map(|&provider| ApiKeyStatus {
                provider,
                has_key: self.has(provider),
            })
            .collect()
    }
}

/// プロダクション実装: OS キーチェーン（keyring）。
pub struct KeyringKeyStore {
    service: String,
}

impl KeyringKeyStore {
    const SERVICE: &'static str = "mitatete-apikeys";

    pub fn new() -> Self {
        Self {
            service: Self::SERVICE.to_string(),
        }
    }

    fn entry(&self, provider: Provider) -> Result<keyring::Entry, ModelError> {
        keyring::Entry::new(&self.service, provider_username(provider))
            .map_err(|e| ModelError::Keyring(e.to_string()))
    }
}

impl Default for KeyringKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyStore for KeyringKeyStore {
    fn set(&self, provider: Provider, key: &str) -> Result<(), ModelError> {
        self.entry(provider)?
            .set_password(key)
            .map_err(|e| ModelError::Keyring(e.to_string()))
    }

    fn get(&self, provider: Provider) -> Result<Option<String>, ModelError> {
        match self.entry(provider)?.get_password() {
            Ok(key) => Ok(Some(key)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(ModelError::Keyring(e.to_string())),
        }
    }
}

// ─── Tauri コマンド ─────────────────────────────────────────────────────────

/// API キーを保存する（設定画面から、要件 3.1）。
#[tauri::command]
pub async fn set_api_key(
    keys: tauri::State<'_, KeyringKeyStore>,
    provider: Provider,
    key: String,
) -> Result<(), ModelError> {
    keys.set(provider, &key)
}

/// 各 provider の API キー有無を返す（**平文キーは返さない**、要件 3.3）。
#[tauri::command]
pub async fn get_api_key_status(
    keys: tauri::State<'_, KeyringKeyStore>,
) -> Result<Vec<ApiKeyStatus>, ModelError> {
    Ok(keys.status_all())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// 実キーチェーンに依存しないモック（HashMap）。
    #[derive(Default)]
    struct MockKeyStore {
        map: Mutex<HashMap<&'static str, String>>,
    }

    impl KeyStore for MockKeyStore {
        fn set(&self, provider: Provider, key: &str) -> Result<(), ModelError> {
            self.map
                .lock()
                .unwrap()
                .insert(provider_username(provider), key.to_string());
            Ok(())
        }
        fn get(&self, provider: Provider) -> Result<Option<String>, ModelError> {
            Ok(self
                .map
                .lock()
                .unwrap()
                .get(provider_username(provider))
                .cloned())
        }
    }

    #[test]
    fn set_then_has_reflects_presence() {
        let store = MockKeyStore::default();
        assert!(!store.has(Provider::Claude));
        store.set(Provider::Claude, "sk-secret").unwrap();
        assert!(store.has(Provider::Claude));
        assert!(!store.has(Provider::Openai));
    }

    #[test]
    fn status_all_reports_presence_without_plaintext() {
        let store = MockKeyStore::default();
        store.set(Provider::Gemini, "g-secret").unwrap();
        let status = store.status_all();
        // 平文キーを含まない（型に key フィールドが無い）。serde 出力にもキー文字列が出ない。
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("g-secret"));
        let gemini = status
            .iter()
            .find(|s| s.provider == Provider::Gemini)
            .unwrap();
        assert!(gemini.has_key);
        let claude = status
            .iter()
            .find(|s| s.provider == Provider::Claude)
            .unwrap();
        assert!(!claude.has_key);
    }
}
