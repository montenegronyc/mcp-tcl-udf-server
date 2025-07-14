use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

#[derive(Clone)]
pub struct AuthConfig {
    pub api_key: String,
    pub require_auth: bool,
}

impl AuthConfig {
    pub fn new() -> Self {
        let api_key = std::env::var("TCL_MCP_API_KEY").unwrap_or_else(|_| {
            warn!("TCL_MCP_API_KEY not set, authentication will be disabled");
            String::new()
        });
        
        let require_auth = !api_key.is_empty() && 
            std::env::var("TCL_MCP_REQUIRE_AUTH")
                .map(|v| v.to_lowercase() != "false")
                .unwrap_or(true);
        
        Self {
            api_key,
            require_auth,
        }
    }
    
    pub fn is_enabled(&self) -> bool {
        self.require_auth && !self.api_key.is_empty()
    }
}

pub async fn auth_middleware(
    State(auth_config): State<AuthConfig>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // Skip authentication if disabled
    if !auth_config.is_enabled() {
        return next.run(request).await;
    }
    
    // Always allow health check endpoints
    let path = request.uri().path();
    if path == "/" || path == "/health" {
        return next.run(request).await;
    }
    
    // Check for API key in headers
    let auth_header = headers.get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    
    let api_key_header = headers.get("X-API-Key")
        .and_then(|h| h.to_str().ok());
    
    let provided_key = auth_header.or(api_key_header);
    
    match provided_key {
        Some(key) if verify_api_key(key, &auth_config.api_key) => {
            debug!("API key authentication successful");
            next.run(request).await
        }
        Some(_) => {
            warn!("Invalid API key provided");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Invalid API key",
                    "message": "The provided API key is invalid or expired"
                }))
            ).into_response()
        }
        None => {
            warn!("No API key provided");
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "Authentication required",
                    "message": "API key required. Provide via 'Authorization: Bearer <key>' or 'X-API-Key: <key>' header"
                }))
            ).into_response()
        }
    }
}

fn verify_api_key(provided_key: &str, expected_key: &str) -> bool {
    // Simple constant-time comparison
    if provided_key.len() != expected_key.len() {
        return false;
    }
    
    provided_key.chars()
        .zip(expected_key.chars())
        .fold(0u8, |acc, (a, b)| acc | (a as u8 ^ b as u8)) == 0
}

pub fn generate_api_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    hex::encode(random_bytes)
}

pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_key_verification() {
        let key = "test-key-123";
        assert!(verify_api_key(key, key));
        assert!(!verify_api_key("wrong-key", key));
        assert!(!verify_api_key("", key));
    }
    
    #[test]
    fn test_api_key_generation() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();
        
        assert_eq!(key1.len(), 64); // 32 bytes * 2 (hex)
        assert_ne!(key1, key2);
    }
    
    #[test]
    fn test_api_key_hashing() {
        let key = "test-key";
        let hash1 = hash_api_key(key);
        let hash2 = hash_api_key(key);
        
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 hex output
    }
}