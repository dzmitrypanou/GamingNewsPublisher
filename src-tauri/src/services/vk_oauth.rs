use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rcgen::generate_simple_self_signed;
use reqwest::Client;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use tokio_rustls::TlsAcceptor;

pub const REDIRECT_URI: &str = "https://localhost";
const CALLBACK_PORT: u16 = 443;
const OAUTH_SCOPE: &str = "wall photos offline";
const AUTH_TIMEOUT: Duration = Duration::from_secs(300);

static ENTROPY_COUNTER: AtomicU64 = AtomicU64::new(0);

const PKCE_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";

pub(crate) struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
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

fn ensure_rustls_provider() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn local_tls_acceptor() -> Result<TlsAcceptor> {
    ensure_rustls_provider();
    let certified = generate_simple_self_signed(vec![
        "127.0.0.1".to_string(),
        "localhost".to_string(),
    ])
    .context("Не удалось создать локальный TLS-сертификат")?;

    let cert = CertificateDer::from(certified.cert.der().to_vec());
    let key = PrivateKeyDer::try_from(certified.key_pair.serialize_der())
        .map_err(|_| anyhow::anyhow!("Не удалось подготовить TLS-ключ"))?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .context("Не удалось настроить TLS для OAuth callback")?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

fn build_authorize_url(app_id: &str, state: &str, code_challenge: &str) -> Result<String> {
    let mut url = reqwest::Url::parse("https://id.vk.ru/authorize")?;
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

fn parse_query(path: &str) -> Result<HashMap<String, String>> {
    let query = path
        .split_once('?')
        .map(|(_, q)| q)
        .unwrap_or(path);
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
    let params = parse_query(path)?;
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

async fn write_callback_response<S>(stream: &mut S, body: &str) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_callback_request<S>(stream: &mut S) -> Result<String>
where
    S: AsyncReadExt + Unpin,
{
    let mut buffer = vec![0u8; 8192];
    let read = stream
        .read(&mut buffer)
        .await
        .context("Не удалось прочитать OAuth callback")?;
    Ok(String::from_utf8_lossy(&buffer[..read]).into_owned())
}

async fn accept_callback(
    listener: TcpListener,
    tls_acceptor: TlsAcceptor,
    expected_state: &str,
) -> Result<CallbackParams> {
    let (stream, _) = listener
        .accept()
        .await
        .context("Не удалось принять OAuth callback")?;

    let mut tls_stream = tls_acceptor
        .accept(stream)
        .await
        .context("TLS handshake OAuth callback не удался")?;

    let request = read_callback_request(&mut tls_stream).await?;
    let request_line = request.lines().next().context("Пустой OAuth callback")?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .context("Некорректный OAuth callback")?;

    let result = parse_callback_path(path, expected_state);
    let body = if result.is_ok() {
        "<!doctype html><html lang=\"ru\"><head><meta charset=\"utf-8\"><title>VK</title></head>\
         <body style=\"font-family:sans-serif;text-align:center;padding:2rem\">\
         <h2>Авторизация VK завершена</h2><p>Можно закрыть это окно и вернуться в приложение.</p>\
         </body></html>"
    } else {
        "<!doctype html><html lang=\"ru\"><head><meta charset=\"utf-8\"><title>VK</title></head>\
         <body style=\"font-family:sans-serif;text-align:center;padding:2rem\">\
         <h2>Ошибка авторизации VK</h2><p>Вернитесь в приложение и попробуйте снова.</p>\
         </body></html>"
    };
    let _ = write_callback_response(&mut tls_stream, body).await;
    result
}

async fn exchange_code(
    client: &Client,
    app_id: &str,
    service_token: &str,
    code_verifier: &str,
    callback: &CallbackParams,
) -> Result<OAuthTokens> {
    let resp = client
        .post("https://id.vk.ru/oauth2/auth")
        .form(&[
            ("grant_type", "authorization_code".to_string()),
            ("code_verifier", code_verifier.to_string()),
            ("redirect_uri", REDIRECT_URI.to_string()),
            ("code", callback.code.clone()),
            ("client_id", app_id.to_string()),
            ("device_id", callback.device_id.clone()),
            ("state", callback.state.clone()),
            ("service_token", service_token.to_string()),
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

pub async fn authorize_user_token(
    app: &AppHandle,
    client: &Client,
    app_id: &str,
    service_token: &str,
) -> Result<OAuthTokens> {
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

    let tls_acceptor = local_tls_acceptor()?;
    let listener = TcpListener::bind(format!("127.0.0.1:{CALLBACK_PORT}"))
        .await
        .context(
            "Не удалось запустить HTTPS-сервер на порту 443. \
             Запустите приложение от имени администратора или освободите порт 443. \
             Альтернатива: получите user token вручную на vkhost.github.io.",
        )?;
    let expected_state = state.clone();
    let callback_task =
        tokio::spawn(async move { accept_callback(listener, tls_acceptor, &expected_state).await });

    app.opener()
        .open_url(authorize_url, None::<&str>)
        .context("Не удалось открыть браузер для авторизации VK")?;

    let callback = match timeout(AUTH_TIMEOUT, callback_task).await {
        Ok(Ok(result)) => result?,
        Ok(Err(err)) => return Err(err).context("OAuth callback failed"),
        Err(_) => bail!("Время ожидания авторизации истекло (5 минут)"),
    };

    exchange_code(
        client,
        app_id,
        service_token,
        &code_verifier,
        &callback,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_uri_uses_https() {
        assert!(REDIRECT_URI.starts_with("https://"));
    }

    #[test]
    fn code_challenge_is_url_safe_base64() {
        let verifier = random_string(64, b"test");
        let challenge = code_challenge(&verifier);
        assert!(challenge.len() >= 43);
        assert!(!challenge.contains('='));
        assert!(!challenge.contains('+'));
        assert!(!challenge.contains('/'));
    }

    #[test]
    fn parses_successful_callback() {
        let state = "expected_state_value_12345678901234567890";
        let path = format!("/callback?code=abc123&device_id=dev456&state={state}");
        let parsed = parse_callback_path(&path, state).unwrap();
        assert_eq!(parsed.code, "abc123");
        assert_eq!(parsed.device_id, "dev456");
    }

    #[test]
    fn rejects_state_mismatch() {
        let path = "/callback?code=abc&device_id=dev&state=wrong";
        assert!(parse_callback_path(path, "expected").is_err());
    }

    #[test]
    fn parses_oauth_error_callback() {
        let path = "/callback?error=access_denied&error_description=User%20denied";
        assert!(parse_callback_path(path, "any").is_err());
    }

    #[test]
    fn local_tls_acceptor_builds() {
        local_tls_acceptor().expect("tls config");
    }
}
