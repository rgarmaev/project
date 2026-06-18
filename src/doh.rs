/// DNS over HTTPS resolver using Cloudflare's DoH endpoint.
/// Bypasses system DNS which may be intercepted by ISP/VPN-level blocking.
use dashmap::DashMap;
use reqwest::Client;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tracing::debug;

const DOH_URL: &str = "https://cloudflare-dns.com/dns-query";
const TTL: Duration = Duration::from_secs(300);

static CACHE: OnceLock<Arc<DashMap<String, (String, Instant)>>> = OnceLock::new();
static CLIENT: OnceLock<Client> = OnceLock::new();

fn cache() -> &'static Arc<DashMap<String, (String, Instant)>> {
    CACHE.get_or_init(|| Arc::new(DashMap::new()))
}

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest client")
    })
}

/// Resolve a hostname to an IPv4 address via Cloudflare DoH.
/// Returns None if DoH fails — caller should fall back to system DNS.
pub async fn resolve(hostname: &str) -> Option<String> {
    if let Some(entry) = cache().get(hostname) {
        let (ip, cached_at) = entry.value().clone();
        if cached_at.elapsed() < TTL {
            debug!("DoH cache hit: {} → {}", hostname, ip);
            return Some(ip);
        }
    }

    let url = format!("{}?name={}&type=A", DOH_URL, hostname);
    let resp = client()
        .get(&url)
        .header("accept", "application/dns-json")
        .send().await.ok()?
        .json::<serde_json::Value>().await.ok()?;

    let ip = resp["Answer"]
        .as_array()?
        .iter()
        .find(|a| a["type"].as_u64() == Some(1))
        .and_then(|a| a["data"].as_str())?
        .to_string();

    cache().insert(hostname.to_string(), (ip.clone(), Instant::now()));
    debug!("DoH resolved: {} → {}", hostname, ip);
    Some(ip)
}

/// Connect a WebSocket using DoH for DNS resolution.
/// Falls back to system DNS if DoH fails.
/// TLS SNI uses the original hostname so certificates validate correctly.
pub async fn connect_ws(
    url: &str,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let request = url.into_client_request()?;
    let host = request.uri().host()
        .ok_or_else(|| anyhow::anyhow!("no host in URL: {}", url))?
        .to_string();
    let port = request.uri().port_u16().unwrap_or(443);

    // Resolve via DoH; fall back to hostname (system DNS) on failure
    let connect_addr = match resolve(&host).await {
        Some(ip) => {
            tracing::info!("DoH: {} → {} (bypassing system DNS)", host, ip);
            format!("{}:{}", ip, port)
        }
        None => {
            tracing::warn!("DoH failed for {}, using system DNS", host);
            format!("{}:{}", host, port)
        }
    };

    let tcp = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::net::TcpStream::connect(&connect_addr),
    ).await
    .map_err(|_| anyhow::anyhow!("TCP connect timeout to {}", connect_addr))??;

    let tls_connector = native_tls::TlsConnector::new()
        .map_err(|e| anyhow::anyhow!("TLS connector: {}", e))?;
    let connector = tokio_tungstenite::Connector::NativeTls(tls_connector);

    let (ws, _) = tokio_tungstenite::client_async_tls_with_config(
        request, tcp, None, Some(connector),
    ).await?;

    Ok(ws)
}
