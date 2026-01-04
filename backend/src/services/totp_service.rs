use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{ImageEncoder, Luma};
use qrcode::QrCode;
use rand::Rng;
use thiserror::Error;
use totp_rs::{Algorithm, Secret, TOTP};

#[derive(Debug, Error)]
pub enum TotpError {
    #[error("Failed to generate QR code")]
    QrCodeGenerationFailed,
    #[error("Invalid secret")]
    InvalidSecret,
    #[error("Hash error: {0}")]
    HashError(String),
}

pub struct TotpService;

impl TotpService {
    /// Generate a new TOTP secret (base32 encoded)
    pub fn generate_secret() -> String {
        let secret = Secret::generate_secret();
        secret.to_encoded().to_string()
    }

    /// Create a TOTP instance from a secret with account name and issuer
    fn create_totp_with_account(secret: &str, username: &str) -> Result<TOTP, TotpError> {
        let secret_bytes = Secret::Encoded(secret.to_string())
            .to_bytes()
            .map_err(|_| TotpError::InvalidSecret)?;

        TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_bytes,
            Some("Dynamight".to_string()),
            username.to_string(),
        )
        .map_err(|_| TotpError::InvalidSecret)
    }

    /// Create a TOTP instance from a secret (for verification only)
    fn create_totp(secret: &str) -> Result<TOTP, TotpError> {
        Self::create_totp_with_account(secret, "user")
    }

    /// Generate the otpauth URL for manual entry
    pub fn get_otpauth_url(username: &str, secret: &str) -> Result<String, TotpError> {
        let totp = Self::create_totp_with_account(secret, username)?;
        Ok(totp.get_url())
    }

    /// Generate QR code as base64-encoded PNG data URL
    pub fn generate_qr_code(username: &str, secret: &str) -> Result<String, TotpError> {
        let otpauth_url = Self::get_otpauth_url(username, secret)?;

        let code = QrCode::new(otpauth_url.as_bytes()).map_err(|_| TotpError::QrCodeGenerationFailed)?;

        let image = code.render::<Luma<u8>>().build();

        let mut png_data = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::L8,
            )
            .map_err(|_| TotpError::QrCodeGenerationFailed)?;

        let base64_png = STANDARD.encode(&png_data);
        Ok(format!("data:image/png;base64,{}", base64_png))
    }

    /// Verify a TOTP code against a secret
    pub fn verify_code(secret: &str, code: &str) -> Result<bool, TotpError> {
        let totp = Self::create_totp(secret)?;
        Ok(totp.check_current(code).unwrap_or(false))
    }

    /// Generate recovery codes (8 codes of format XXXX-XXXX-XXXX)
    pub fn generate_recovery_codes() -> Vec<String> {
        let mut rng = rand::thread_rng();
        (0..8)
            .map(|_| {
                let parts: Vec<String> = (0..3)
                    .map(|_| {
                        (0..4)
                            .map(|_| {
                                if rng.gen_bool(0.5) {
                                    rng.gen_range('A'..='Z')
                                } else {
                                    rng.gen_range('0'..='9')
                                }
                            })
                            .collect::<String>()
                    })
                    .collect();
                parts.join("-")
            })
            .collect()
    }

    /// Hash a recovery code using Argon2
    pub fn hash_recovery_code(code: &str) -> Result<String, TotpError> {
        let normalized = code.to_uppercase().replace('-', "");
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(normalized.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| TotpError::HashError(e.to_string()))
    }

    /// Verify a recovery code against a hash
    pub fn verify_recovery_code(code: &str, hash: &str) -> Result<bool, TotpError> {
        let normalized = code.to_uppercase().replace('-', "");
        let parsed_hash =
            PasswordHash::new(hash).map_err(|e| TotpError::HashError(e.to_string()))?;

        Ok(Argon2::default()
            .verify_password(normalized.as_bytes(), &parsed_hash)
            .is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_secret() {
        let secret = TotpService::generate_secret();
        assert!(!secret.is_empty());
        // Base32 encoded secrets should only contain A-Z and 2-7
        assert!(secret.chars().all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c)));
    }

    #[test]
    fn test_verify_code() {
        let secret = TotpService::generate_secret();
        // Get the current valid code
        let totp = TotpService::create_totp_with_account(&secret, "test").unwrap();
        let valid_code = totp.generate_current().unwrap();

        assert!(TotpService::verify_code(&secret, &valid_code).unwrap());
        assert!(!TotpService::verify_code(&secret, "000000").unwrap());
    }

    #[test]
    fn test_recovery_codes() {
        let codes = TotpService::generate_recovery_codes();
        assert_eq!(codes.len(), 8);

        for code in &codes {
            // Format: XXXX-XXXX-XXXX
            assert_eq!(code.len(), 14);
            assert_eq!(code.chars().filter(|&c| c == '-').count(), 2);
        }
    }

    #[test]
    fn test_recovery_code_hash_verify() {
        let codes = TotpService::generate_recovery_codes();
        let code = &codes[0];

        let hash = TotpService::hash_recovery_code(code).unwrap();
        assert!(TotpService::verify_recovery_code(code, &hash).unwrap());

        // Should also work with lowercase and no dashes
        let normalized = code.to_lowercase().replace('-', "");
        assert!(TotpService::verify_recovery_code(&normalized, &hash).unwrap());
    }
}
