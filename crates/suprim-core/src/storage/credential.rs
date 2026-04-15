//! Credential encryption — AES-256-GCM with machine-derived key.
//!
//! Passwords stored on disk are prefixed with `enc:` followed by
//! base64(nonce ++ ciphertext). Plain text values (legacy) are
//! detected by absence of the prefix and auto-migrated on save.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce};
use base64::prelude::*;
use sha2::{Digest, Sha256};

const ENC_PREFIX: &str = "enc:";

/// Derive a 256-bit AES key from the machine's unique identifier.
fn derive_key() -> Key<Aes256Gcm> {
    let uid = machine_uid::get().unwrap_or_else(|_| "suprim-fallback-key".to_string());
    let hash = Sha256::digest(uid.as_bytes());
    Key::<Aes256Gcm>::try_from(hash.as_slice()).expect("SHA-256 produces 32 bytes")
}

/// Encrypt a plain-text password → `"enc:<base64(nonce ++ ciphertext)>"`.
pub fn encrypt(plain: &str) -> String {
    if plain.is_empty() {
        return String::new();
    }
    let key = derive_key();
    let cipher = Aes256Gcm::new(&key);

    let mut nonce_bytes = [0u8; 12];
    rand::fill(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, plain.as_bytes())
        .expect("AES-GCM encryption should not fail");

    // nonce (12 bytes) ++ ciphertext
    let mut blob = nonce_bytes.to_vec();
    blob.extend_from_slice(&ciphertext);

    format!("{ENC_PREFIX}{}", BASE64_STANDARD.encode(&blob))
}

/// Decrypt an `"enc:..."` value back to plain text.
/// If the value is NOT prefixed (legacy plain text), returns it as-is.
pub fn decrypt(stored: &str) -> String {
    if stored.is_empty() {
        return String::new();
    }
    let Some(encoded) = stored.strip_prefix(ENC_PREFIX) else {
        // Legacy plain text — return as-is
        return stored.to_string();
    };

    let blob = match BASE64_STANDARD.decode(encoded) {
        Ok(b) => b,
        Err(_) => {
            tracing::warn!("Failed to base64-decode credential, treating as plain text");
            return stored.to_string();
        }
    };

    if blob.len() < 13 {
        // nonce(12) + at least 1 byte ciphertext
        tracing::warn!("Credential blob too short, treating as plain text");
        return stored.to_string();
    }

    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let nonce = Nonce::try_from(nonce_bytes).expect("nonce is 12 bytes");
    let key = derive_key();
    let cipher = Aes256Gcm::new(&key);

    match cipher.decrypt(&nonce, ciphertext) {
        Ok(plaintext) => String::from_utf8(plaintext).unwrap_or_else(|_| {
            tracing::warn!("Decrypted credential is not valid UTF-8");
            stored.to_string()
        }),
        Err(_) => {
            tracing::warn!("Failed to decrypt credential (wrong machine?), treating as plain text");
            stored.to_string()
        }
    }
}

/// Returns `true` if the value is already encrypted.
pub fn is_encrypted(stored: &str) -> bool {
    stored.starts_with(ENC_PREFIX)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let password = "my-secret-p@ssw0rd!";
        let encrypted = encrypt(password);
        assert!(encrypted.starts_with(ENC_PREFIX));
        assert_ne!(encrypted, password);
        let decrypted = decrypt(&encrypted);
        assert_eq!(decrypted, password);
    }

    #[test]
    fn empty_string() {
        assert_eq!(encrypt(""), "");
        assert_eq!(decrypt(""), "");
    }

    #[test]
    fn legacy_plain_text_passthrough() {
        let plain = "old-plain-password";
        assert_eq!(decrypt(plain), plain);
    }

    #[test]
    fn is_encrypted_check() {
        assert!(!is_encrypted("plain-text"));
        assert!(is_encrypted(&encrypt("secret")));
    }

    #[test]
    fn different_encryptions_produce_different_output() {
        let a = encrypt("same");
        let b = encrypt("same");
        // Different nonces → different ciphertext
        assert_ne!(a, b);
        // Both decrypt to same value
        assert_eq!(decrypt(&a), "same");
        assert_eq!(decrypt(&b), "same");
    }
}
