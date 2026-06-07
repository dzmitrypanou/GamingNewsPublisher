use crate::models::{ApiTestResult, AppSettings};
use anyhow::{bail, Context, Result};
use reqwest::{Client, Proxy};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

pub struct HttpClientPool {
    clients: Vec<Client>,
    index: AtomicUsize,
}

impl HttpClientPool {
    pub fn from_settings(settings: &AppSettings) -> Result<Self> {
        let direct = build_direct_client()?;

        if !settings.proxy_enabled {
            return Ok(Self {
                clients: vec![direct],
                index: AtomicUsize::new(0),
            });
        }

        let scheme = normalize_proxy_type(&settings.proxy_type);
        let mut clients = Vec::new();

        for line in settings.proxy_list.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match parse_proxy_line(line, scheme) {
                Ok(url) => match build_proxied_client(&url) {
                    Ok(client) => clients.push(client),
                    Err(e) => eprintln!("Прокси «{}»: {}", line, e),
                },
                Err(e) => eprintln!("Прокси «{}»: {}", line, e),
            }
        }

        if clients.is_empty() {
            bail!(
                "Прокси включён, но ни одна строка в списке не распознана. \
                 Проверьте формат (IP:PORT, IP:PORT@LOGIN:PASS и т.д.)"
            );
        }

        Ok(Self {
            clients,
            index: AtomicUsize::new(0),
        })
    }

    pub fn next(&self) -> Client {
        if self.clients.len() == 1 {
            return self.clients[0].clone();
        }
        let idx = self.index.fetch_add(1, Ordering::Relaxed) % self.clients.len();
        self.clients[idx].clone()
    }

}

pub fn rebuild_pool(
    pool: &Mutex<HttpClientPool>,
    settings: &AppSettings,
) -> Result<()> {
    let next = HttpClientPool::from_settings(settings)?;
    *pool.lock().map_err(|e| anyhow::anyhow!("http pool lock: {}", e))? = next;
    Ok(())
}

pub async fn test_proxy_connection(client: &Client) -> ApiTestResult {
    match client
        .get("https://api.ipify.org?format=json")
        .header(
            "User-Agent",
            "Mozilla/5.0 GamingNewsPublisher/0.1",
        )
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            let body = response.text().await.unwrap_or_default();
            let ip = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("ip").and_then(|ip| ip.as_str()).map(String::from))
                .unwrap_or(body);
            ApiTestResult {
                success: true,
                message: format!("Подключение успешно. Внешний IP: {}", ip),
            }
        }
        Ok(response) => ApiTestResult {
            success: false,
            message: format!("HTTP {}", response.status()),
        },
        Err(e) => ApiTestResult {
            success: false,
            message: format!("Ошибка: {}", e),
        },
    }
}

fn build_direct_client() -> Result<Client> {
    Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .context("Не удалось создать HTTP-клиент")
}

fn build_proxied_client(proxy_url: &str) -> Result<Client> {
    let proxy = Proxy::all(proxy_url).with_context(|| format!("Некорректный прокси: {}", proxy_url))?;
    Client::builder()
        .timeout(HTTP_TIMEOUT)
        .proxy(proxy)
        .build()
        .context("Не удалось создать HTTP-клиент с прокси")
}

fn normalize_proxy_type(proxy_type: &str) -> &str {
    match proxy_type.trim().to_lowercase().as_str() {
        "https" => "https",
        "socks5" | "socks" => "socks5",
        _ => "http",
    }
}

/// Поддерживаемые форматы (по одному на строку):
/// - `IP:PORT`
/// - `IP:PORT@LOGIN:PASS`
/// - `LOGIN:PASS@IP:PORT`
/// - `IP:PORT:LOGIN:PASS`
/// - `LOGIN:PASS:IP:PORT`
/// - `http(s)://...` / `socks5://...` (схема в строке имеет приоритет над типом в настройках)
pub fn parse_proxy_line(line: &str, default_scheme: &str) -> Result<String> {
    let line = line.trim();
    if line.is_empty() {
        bail!("пустая строка");
    }

    if line.contains("://") {
        return Ok(line.to_string());
    }

    if let Some((left, right)) = line.split_once('@') {
        if looks_like_host_port(left) {
            let (host, port) = split_host_port(left)?;
            let (user, pass) = split_credentials(right)?;
            return Ok(format_proxy_url(default_scheme, host, port, Some(user), Some(pass)));
        }
        if looks_like_host_port(right) {
            let (user, pass) = split_credentials(left)?;
            let (host, port) = split_host_port(right)?;
            return Ok(format_proxy_url(default_scheme, host, port, Some(user), Some(pass)));
        }
        bail!("не удалось распознать формат с @");
    }

    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() == 4 {
        if parts[1].chars().all(|c| c.is_ascii_digit()) {
            return Ok(format_proxy_url(
                default_scheme,
                parts[0],
                parts[1],
                Some(parts[2]),
                Some(parts[3]),
            ));
        }
        if parts[3].chars().all(|c| c.is_ascii_digit()) {
            return Ok(format_proxy_url(
                default_scheme,
                parts[2],
                parts[3],
                Some(parts[0]),
                Some(parts[1]),
            ));
        }
    }

    if parts.len() == 2 && parts[1].chars().all(|c| c.is_ascii_digit()) {
        return Ok(format_proxy_url(default_scheme, parts[0], parts[1], None, None));
    }

    bail!(
        "неизвестный формат. Поддерживаются: IP:PORT, IP:PORT@LOGIN:PASS, LOGIN:PASS@IP:PORT, \
         IP:PORT:LOGIN:PASS, http(s)://..., socks5://..."
    );
}

fn looks_like_host_port(value: &str) -> bool {
    split_host_port(value).is_ok()
}

fn split_host_port(value: &str) -> Result<(&str, &str)> {
    let value = value.trim();
    if value.starts_with('[') {
        let end = value.find(']').context("некорректный IPv6")?;
        let host = &value[1..end];
        let rest = value.get(end + 1..).unwrap_or("");
        if !rest.starts_with(':') {
            bail!("ожидается :PORT после IPv6");
        }
        let port = rest.trim_start_matches(':');
        if !port.chars().all(|c| c.is_ascii_digit()) {
            bail!("некорректный порт");
        }
        return Ok((host, port));
    }

    let (host, port) = value
        .rsplit_once(':')
        .context("ожидается HOST:PORT")?;
    if host.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        bail!("некорректный HOST:PORT");
    }
    Ok((host, port))
}

fn split_credentials(value: &str) -> Result<(&str, &str)> {
    let (user, pass) = value
        .split_once(':')
        .context("ожидается LOGIN:PASS")?;
    if user.is_empty() || pass.is_empty() {
        bail!("пустой логин или пароль");
    }
    Ok((user, pass))
}

fn format_proxy_url(
    scheme: &str,
    host: &str,
    port: &str,
    user: Option<&str>,
    pass: Option<&str>,
) -> String {
    let host_part = if host.contains(':') {
        format!("[{}]", host)
    } else {
        host.to_string()
    };

    match (user, pass) {
        (Some(user), Some(pass)) => format!("{scheme}://{user}:{pass}@{host_part}:{port}"),
        _ => format!("{scheme}://{host_part}:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_port() {
        let url = parse_proxy_line("192.168.1.1:8080", "http").unwrap();
        assert_eq!(url, "http://192.168.1.1:8080");
    }

    #[test]
    fn parses_host_port_at_credentials() {
        let url = parse_proxy_line("10.0.0.2:3128@user:secret", "socks5").unwrap();
        assert_eq!(url, "socks5://user:secret@10.0.0.2:3128");
    }

    #[test]
    fn parses_credentials_at_host_port() {
        let url = parse_proxy_line("user:secret@10.0.0.2:3128", "https").unwrap();
        assert_eq!(url, "https://user:secret@10.0.0.2:3128");
    }

    #[test]
    fn parses_four_part_colon_format() {
        let url = parse_proxy_line("10.0.0.2:3128:user:secret", "http").unwrap();
        assert_eq!(url, "http://user:secret@10.0.0.2:3128");
    }

    #[test]
    fn parses_explicit_scheme() {
        let url = parse_proxy_line("socks5://1.2.3.4:1080", "http").unwrap();
        assert_eq!(url, "socks5://1.2.3.4:1080");
    }
}
