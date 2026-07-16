//! PKCE (RFC 7636) S256 verifier / challenge and OAuth `state` generation.

use {
    base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD},
    rand::Rng,
    sha2::{Digest, Sha256},
};

/// High-entropy PKCE pair plus opaque CSRF `state`.
#[derive(Clone, Debug)]
pub struct PkceSession {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

/// Generate a new PKCE S256 session (`code_verifier`, `code_challenge`, `state`).
pub fn generate_pkce_session() -> PkceSession {
    let verifier = random_url_safe(32);
    let challenge = s256_challenge(&verifier);
    let state = random_url_safe(16);
    PkceSession {
        verifier,
        challenge,
        state,
    }
}

/// BASE64URL-ENCODE(SHA256(ASCII(code_verifier))) without padding.
pub fn s256_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn random_url_safe(nbytes: usize) -> String {
    let mut buf = vec![0u8; nbytes];
    () = rand::rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}
