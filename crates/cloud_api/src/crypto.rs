use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce}; // Or `Aes128Gcm`
use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;

const NONCE_LEN: usize = 12;
const KEY_ENV: &str = "DELIVERY_CRED_ENC_KEY";

fn get_cipher() -> Result<Aes256Gcm, String> {
    let key_b64 = std::env::var(KEY_ENV)
        .map_err(|_| format!("missing {} env var for delivery credential encryption", KEY_ENV))?;
    let key_bytes = general_purpose::STANDARD
        .decode(key_b64)
        .map_err(|_| format!("{} must be base64-encoded 32-byte key", KEY_ENV))?;
    if key_bytes.len() != 32 {
        return Err(format!("{} must decode to 32 bytes", KEY_ENV));
    }
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|_| "failed to construct AES-256-GCM cipher".to_string())?;
    Ok(cipher)
}

/// Encrypt a secret string using AES-256-GCM and return base64(nonce || ciphertext).
pub fn encrypt_secret(plaintext: &str) -> Result<String, String> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }
    let cipher = get_cipher()?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("encrypt error: {e}"))?;
    let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(general_purpose::STANDARD.encode(combined))
}

/// Decrypt a secret string that was produced by `encrypt_secret`.
pub fn decrypt_secret(ciphertext_b64: &str) -> Result<String, String> {
    if ciphertext_b64.is_empty() {
        return Ok(String::new());
    }
    let cipher = get_cipher()?;
    let combined = general_purpose::STANDARD
        .decode(ciphertext_b64)
        .map_err(|_| "ciphertext must be valid base64".to_string())?;
    if combined.len() <= NONCE_LEN {
        return Err("ciphertext too short".to_string());
    }
    let (nonce_bytes, ct) = combined.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ct)
        .map_err(|e| format!("decrypt error: {e}"))?;
    String::from_utf8(plaintext).map_err(|_| "decrypted secret is not UTF-8".to_string())
}

