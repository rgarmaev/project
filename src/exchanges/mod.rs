pub mod binance;
pub mod bybit;
pub mod mexc;
pub mod okx;
pub mod bitget;
pub mod kucoin;
pub mod gate;

use base64::Engine;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn sign_hmac_sha256(secret: &str, payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// OKX signing: base64(HMAC-SHA256(timestamp + method + path + body))
pub fn sign_hmac_sha256_b64(secret: &str, payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(payload.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

pub fn now_ms() -> u64 {
    Utc::now().timestamp_millis() as u64
}
