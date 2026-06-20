use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

pub const REDIRECT_URI: &str = "https://oauth.vk.com/blank.html";
const OAUTH_SCOPE: &str = "wall,photos,offline,groups";
const AUTHORIZE_URL: &str = "https://id.vk.com/authorize";
const TOKEN_URL: &str = "https://id.vk.com/oauth2/auth";

static ENTROPY_COUNTER: AtomicU64 = AtomicU64::new(0);

const PKCE_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";

pub(crate) struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
}

pub struct PendingVkOAuth {
    pub code_verifier: String,
    pub state: String,
    pub app_id: String,
    pub service_token: String,
}

struct CallbackParams {
    code: String,
    device_id: String,
    state: String,
}

fn random_string(len: usize, label: &[u8]) -> String {
    let mut out = Vec::with_capacity(len);
    let mut round = 0u64;
    while out.len() < len {
        let mut hasher = Sha256::new();
        hasher.update(label);
        hasher.update(round.to_le_bytes());
        hasher.update(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                .to_le_bytes(),
        );
        hasher.update(ENTROPY_COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
        hasher.update(std::process::id().to_le_bytes());
        for byte in hasher.finalize() {
            if out.len() >= len {
                break;
            }
            out.push(PKCE_CHARS[(byte as usize) % PKCE_CHARS.len()]);
        }
        round += 1;
    }
    String::from_utf8(out).expect("pkce charset is ascii")
}

fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn build_authorize_url(app_id: &str, state: &str, code_challenge: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(AUTHORIZE_URL)?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", app_id)
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("scope", OAUTH_SCOPE);
    Ok(url.to_string())
}

fn parse_query(query: &str) -> Result<HashMap<String, String>> {
    let query = query.trim_start_matches('?');
    let mut params = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair
            .split_once('=')
            .map(|(k, v)| (k, v))
            .unwrap_or((pair, ""));
        params.insert(urlencoding_decode(key), urlencoding_decode(value));
    }
    Ok(params)
}

fn urlencoding_decode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &input[i + 1..i + 3];
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value as char);
                }
                i += 3;
            }
            ch => {
                out.push(ch as char);
                i += 1;
            }
        }
    }
    out
}

fn parse_callback_path(path: &str, expected_state: &str) -> Result<CallbackParams> {
    let params = parse_query(
        path
            .split_once('?')
            .map(|(_, q)| q)
            .unwrap_or(path.trim_start_matches('/')),
    )?;
    if let Some(error) = params.get("error") {
        let description = params
            .get("error_description")
            .map(String::as_str)
            .unwrap_or(error);
        bail!("VK отклонил авторизацию: {description}");
    }

    let state = params
        .get("state")
        .context("VK не вернул state")?
        .clone();
    if state != expected_state {
        bail!("Неверный state в ответе VK — возможна подмена запроса");
    }

    Ok(CallbackParams {
        code: params.get("code").context("VK не вернул code")?.clone(),
        device_id: params
            .get("device_id")
            .context("VK не вернул device_id")?
            .clone(),
        state,
    })
}

pub fn parse_callback_from_pasted(raw: &str, expected_state: &str) -> Result<CallbackParams> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("Вставьте URL из адресной строки браузера после входа VK");
    }

    if raw.contains("://") {
        let url = reqwest::Url::parse(raw).context("Некорректный URL")?;
        let mut path = url.path().to_string();
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        } else if url.path().is_empty() || url.path() == "/" {
            if let Some(fragment) = url.fragment() {
                if fragment.contains('=') {
                    path = format!("/?{fragment}");
                }
            }
        }
        return parse_callback_path(&path, expected_state);
    }

    if raw.starts_with('?') {
        return parse_callback_path(raw, expected_state);
    }

    if raw.contains('=') {
        return parse_callback_path(&format!("?{raw}"), expected_state);
    }

    bail!("Не удалось распознать code и device_id. Скопируйте полный адрес из строки браузера.");
}

pub fn begin_oauth(
    app: &AppHandle,
    app_id: &str,
    service_token: &str,
) -> Result<(PendingVkOAuth, String)> {
    let app_id = app_id.trim();
    let service_token = service_token.trim();

    if app_id.is_empty() {
        bail!("Укажите ID приложения VK");
    }
    if service_token.is_empty() {
        bail!("Укажите сервисный ключ доступа VK");
    }

    let code_verifier = random_string(64, b"verifier");
    let challenge = code_challenge(&code_verifier);
    let state = random_string(48, b"state");
    let authorize_url = build_authorize_url(app_id, &state, &challenge)?;

    let pending = PendingVkOAuth {
        code_verifier,
        state,
        app_id: app_id.to_string(),
        service_token: service_token.to_string(),
    };

    app.opener()
        .open_url(&authorize_url, None::<&str>)
        .context("Не удалось открыть браузер для авторизации VK")?;

    Ok((pending, authorize_url))
}

async fn exchange_code(
    client: &Client,
    pending: &PendingVkOAuth,
    callback: &CallbackParams,
) -> Result<OAuthTokens> {
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code".to_string()),
            ("code_verifier", pending.code_verifier.clone()),
            ("redirect_uri", REDIRECT_URI.to_string()),
            ("code", callback.code.clone()),
            ("client_id", pending.app_id.clone()),
            ("device_id", callback.device_id.clone()),
            ("state", callback.state.clone()),
            ("service_token", pending.service_token.clone()),
        ])
        .send()
        .await
        .context("VK OAuth: сеть недоступна")?;

    let value: Value = resp
        .json()
        .await
        .context("VK OAuth: некорректный ответ")?;

    if let Some(error) = value.get("error") {
        let msg = value["error_description"]
            .as_str()
            .or_else(|| error.as_str())
            .unwrap_or("OAuth error");
        bail!("{msg}");
    }

    let access_token = value["access_token"]
        .as_str()
        .context("VK OAuth: нет access_token")?
        .to_string();
    let refresh_token = value["refresh_token"].as_str().unwrap_or("").to_string();

    Ok(OAuthTokens {
        access_token,
        refresh_token,
    })
}

pub async fn finish_oauth(
    client: &Client,
    pending: PendingVkOAuth,
    pasted_url: &str,
) -> Result<OAuthTokens> {
    let callback = parse_callback_from_pasted(pasted_url, &pending.state)?;
    exchange_code(client, &pending, &callback).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri_uses_https_blank() {
        assert_eq!(REDIRECT_URI, "https://oauth.vk.com/blank.html");
    }

    #[test]
    fn code_challenge_is_url_safe_base64() {
        let verifier = random_string(64, b"test");
        let challenge = code_challenge(&verifier);
        assert!(challenge.len() >= 43);
        assert!(!challenge.contains('='));
    }

    #[test]
    fn parses_pasted_localhost_url() {
        let state = "expected_state_value_12345678901234567890";
        let url = format!("https://oauth.vk.com/blank.html?code=abc123&device_id=dev456&state={state}");
        let parsed = parse_callback_from_pasted(&url, state).unwrap();
        assert_eq!(parsed.code, "abc123");
        assert_eq!(parsed.device_id, "dev456");
    }

    #[test]
    fn parses_query_only_paste() {
        let state = "expected_state_value_12345678901234567890";
        let query = format!("code=abc123&device_id=dev456&state={state}");
        let parsed = parse_callback_from_pasted(&query, state).unwrap();
        assert_eq!(parsed.code, "abc123");
    }

    #[test]
    fn rejects_state_mismatch() {
        let url = "https://localhost/?code=abc&device_id=dev&state=wrong";
        assert!(parse_callback_from_pasted(url, "expected").is_err());
    }
}
