//! PKCE (Proof Key for Code Exchange) implementation
//!
//! Implements RFC 7636 for secure OAuth 2.0 authorization code flow.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use sha2::{Digest, Sha256};

/// PKCE challenge/verifier pair
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    /// The code verifier (43-128 chars, [A-Za-z0-9-._~])
    pub verifier: String,
    /// The S256 code challenge (base64url(sha256(verifier)))
    pub challenge: String,
}

impl PkceChallenge {
    /// Generate a new PKCE challenge/verifier pair
    pub fn new() -> Self {
        let verifier = generate_code_verifier();
        let challenge = generate_code_challenge(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}

impl Default for PkceChallenge {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a cryptographically random code verifier
///
/// The verifier is 43-128 characters from the set [A-Za-z0-9-._~]
/// We use 64 characters for good security.
pub fn generate_code_verifier() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    const VERIFIER_LENGTH: usize = 64;

    let mut rng = rand::thread_rng();
    (0..VERIFIER_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate the S256 code challenge from a verifier
///
/// challenge = base64url(sha256(verifier))
pub fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    URL_SAFE_NO_PAD.encode(hash)
}

/// Generate a random state string for CSRF protection
pub fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    URL_SAFE_NO_PAD.encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_length() {
        let verifier = generate_code_verifier();
        assert!(verifier.len() >= 43 && verifier.len() <= 128);
    }

    #[test]
    fn test_verifier_charset() {
        let verifier = generate_code_verifier();
        for c in verifier.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~',
                "Invalid character in verifier: {}",
                c
            );
        }
    }

    #[test]
    fn test_challenge_is_base64url() {
        let verifier = generate_code_verifier();
        let challenge = generate_code_challenge(&verifier);
        // S256 challenge should be 43 characters (256 bits / 6 bits per char)
        assert_eq!(challenge.len(), 43);
    }

    #[test]
    fn test_state_generation() {
        let state1 = generate_state();
        let state2 = generate_state();
        assert_ne!(state1, state2); // Should be random
        assert!(!state1.is_empty());
    }
}
