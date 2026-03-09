use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid token")]
    InvalidToken,
    #[error("Hash error: {0}")]
    HashError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64, // user_id
    pub exp: usize,
    pub iat: usize,
    pub jti: String,
}

#[derive(Clone)]
pub struct AuthService {
    jwt_secret: String,
}

impl AuthService {
    pub fn new(jwt_secret: String) -> Self {
        Self { jwt_secret }
    }

    pub fn hash_password(password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| AuthError::HashError(e.to_string()))
    }

    pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| AuthError::HashError(e.to_string()))?;

        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    pub fn generate_token(&self, user_id: i64) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = now + Duration::hours(24);

        let claims = Claims {
            sub: user_id,
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
            jti: uuid::Uuid::new_v4().to_string(),
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|_| AuthError::InvalidToken)
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map(|data| data.claims)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            _ => AuthError::InvalidToken,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let password = "my_secure_password_123!";
        let hash = AuthService::hash_password(password).unwrap();
        assert!(AuthService::verify_password(password, &hash).unwrap());
    }

    #[test]
    fn wrong_password_returns_false() {
        let hash = AuthService::hash_password("correct_password").unwrap();
        assert!(!AuthService::verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn hash_produces_unique_salts() {
        let password = "same_password";
        let hash1 = AuthService::hash_password(password).unwrap();
        let hash2 = AuthService::hash_password(password).unwrap();
        assert_ne!(hash1, hash2, "Each hash should use a unique salt");
        // Both should still verify
        assert!(AuthService::verify_password(password, &hash1).unwrap());
        assert!(AuthService::verify_password(password, &hash2).unwrap());
    }

    #[test]
    fn verify_password_with_invalid_hash_returns_error() {
        let result = AuthService::verify_password("password", "not_a_valid_hash");
        assert!(result.is_err());
    }

    #[test]
    fn token_generate_and_validate_roundtrip() {
        let service = AuthService::new("test_secret_key".to_string());
        let user_id = 42;
        let token = service.generate_token(user_id).unwrap();
        let claims = service.validate_token(&token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert!(!claims.jti.is_empty());
    }

    #[test]
    fn token_claims_have_correct_expiry() {
        let service = AuthService::new("test_secret".to_string());
        let token = service.generate_token(1).unwrap();
        let claims = service.validate_token(&token).unwrap();
        // exp should be ~24 hours after iat
        let diff = claims.exp - claims.iat;
        assert!((86300..=86400).contains(&diff), "Expected ~24h expiry, got {} seconds", diff);
    }

    #[test]
    fn expired_token_returns_token_expired() {
        let secret = "test_secret";
        let service = AuthService::new(secret.to_string());

        // Manually craft an expired token
        let claims = Claims {
            sub: 1,
            exp: 1000, // long in the past
            iat: 500,
            jti: "test-jti".to_string(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap();

        let result = service.validate_token(&token);
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    #[test]
    fn invalid_token_string_returns_invalid_token() {
        let service = AuthService::new("test_secret".to_string());
        let result = service.validate_token("not.a.valid.jwt.token");
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }

    #[test]
    fn different_secret_rejects_token() {
        let service1 = AuthService::new("secret_one".to_string());
        let service2 = AuthService::new("secret_two".to_string());

        let token = service1.generate_token(1).unwrap();
        let result = service2.validate_token(&token);
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }
}
