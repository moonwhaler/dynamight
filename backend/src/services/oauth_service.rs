//! OAuth2 access-token refresh for Google Drive and OneDrive credentials.
//!
//! Google and Microsoft access tokens expire after about an hour, so scheduled
//! backups need to renew them with the stored refresh token. The refresh uses
//! the user's own OAuth app registration (client ID / secret stored in the
//! credential), see:
//! - Google: <https://developers.google.com/identity/protocols/oauth2/web-server#offline>
//! - Microsoft: <https://learn.microsoft.com/entra/identity-platform/v2-oauth2-auth-code-flow#refresh-the-access-token>

use crate::models::CredentialData;
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::Deserialize;

const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const MICROSOFT_TOKEN_URL_TEMPLATE: &str = "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token";
const DEFAULT_MICROSOFT_TENANT: &str = "common";
pub const DEFAULT_MICROSOFT_SCOPE: &str = "offline_access Files.ReadWrite.All";

/// Refresh the access token this many seconds before it expires.
pub const REFRESH_MARGIN_SECS: i64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    Google,
    Microsoft,
}

impl OAuthProvider {
    /// Map a credential `provider_type` to its OAuth provider.
    pub fn from_provider_type(provider_type: &str) -> Option<Self> {
        match provider_type {
            "google_drive" => Some(Self::Google),
            "onedrive" => Some(Self::Microsoft),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenErrorResponse {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

/// True if the OAuth credential's access token is missing or expires soon.
pub fn needs_refresh(data: &CredentialData, now: i64) -> bool {
    match data {
        CredentialData::OAuth { access_token, expires_at, .. } => {
            access_token.is_empty() || *expires_at - now < REFRESH_MARGIN_SECS
        }
        _ => false,
    }
}

/// True if the credential carries what is needed to refresh its access token.
pub fn can_refresh(provider: OAuthProvider, data: &CredentialData) -> bool {
    match data {
        CredentialData::OAuth { refresh_token, client_id, client_secret, .. } => {
            let has = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.is_empty());
            !refresh_token.is_empty()
                && has(client_id)
                && (provider == OAuthProvider::Microsoft || has(client_secret))
        }
        _ => false,
    }
}

/// Build the form parameters for a refresh-token request.
fn refresh_form(provider: OAuthProvider, data: &CredentialData) -> Result<Vec<(&'static str, String)>> {
    let CredentialData::OAuth { refresh_token, client_id, client_secret, scope, .. } = data else {
        return Err(anyhow!("Not an OAuth credential"));
    };
    let client_id = client_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("OAuth client ID is not set on this credential"))?;

    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh_token.clone()),
        ("client_id", client_id.to_string()),
    ];
    if let Some(secret) = client_secret.as_deref().filter(|s| !s.is_empty()) {
        form.push(("client_secret", secret.to_string()));
    } else if provider == OAuthProvider::Google {
        return Err(anyhow!("Google requires the OAuth client secret to refresh tokens"));
    }
    if provider == OAuthProvider::Microsoft {
        let scope = scope
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_MICROSOFT_SCOPE);
        form.push(("scope", scope.to_string()));
    }
    Ok(form)
}

fn token_url(provider: OAuthProvider, data: &CredentialData) -> String {
    match provider {
        OAuthProvider::Google => GOOGLE_TOKEN_URL.to_string(),
        OAuthProvider::Microsoft => {
            let tenant = match data {
                CredentialData::OAuth { tenant: Some(t), .. } if !t.trim().is_empty() => t.trim().to_string(),
                _ => DEFAULT_MICROSOFT_TENANT.to_string(),
            };
            // Tenant goes into the URL path: only allow the characters a tenant
            // name, domain or GUID can contain.
            let tenant: String = tenant
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
                .collect();
            MICROSOFT_TOKEN_URL_TEMPLATE.replace("{tenant}", &tenant)
        }
    }
}

/// Turn a token endpoint error into a message that tells the user what to do.
fn describe_token_error(provider: OAuthProvider, status: u16, body: &str) -> String {
    let parsed: Option<TokenErrorResponse> = serde_json::from_str(body).ok();
    let (code, desc) = match &parsed {
        Some(e) => (e.error.as_str(), e.error_description.clone().unwrap_or_default()),
        None => ("", body.chars().take(300).collect()),
    };

    let hint = match (provider, code) {
        (OAuthProvider::Google, "invalid_grant") => {
            " The refresh token is expired or revoked. Create a new one. Note: while the \
             Google Cloud OAuth consent screen is in \"Testing\" status, refresh tokens expire \
             after 7 days; set it to \"In production\" to avoid this."
        }
        (OAuthProvider::Microsoft, "invalid_grant") => {
            " The refresh token is expired or revoked (or the scope does not match the \
             original consent). Sign in again to get a new refresh token."
        }
        (_, "invalid_client") | (_, "unauthorized_client") => {
            " Check the client ID and client secret of the credential."
        }
        _ => "",
    };

    if code.is_empty() {
        format!("Token refresh failed (HTTP {}): {}", status, desc)
    } else {
        format!("Token refresh failed (HTTP {}): {} {}.{}", status, code, desc, hint)
    }
}

/// Apply a successful token response to the credential data.
fn apply_token_response(data: &mut CredentialData, response: TokenResponse, now: i64) {
    if let CredentialData::OAuth { access_token, refresh_token, expires_at, .. } = data {
        *access_token = response.access_token;
        *expires_at = now + response.expires_in.unwrap_or(3600);
        // Microsoft rotates refresh tokens; Google normally keeps the old one.
        if let Some(new_refresh) = response.refresh_token.filter(|t| !t.is_empty()) {
            *refresh_token = new_refresh;
        }
    }
}

/// Exchange the refresh token for a new access token and update `data` in place.
pub async fn refresh_access_token(
    client: &reqwest::Client,
    provider: OAuthProvider,
    data: &mut CredentialData,
) -> Result<()> {
    let form = refresh_form(provider, data)?;
    let url = token_url(provider, data);

    let response = client
        .post(&url)
        .form(&form)
        .send()
        .await
        .map_err(|e| anyhow!("Token refresh request failed: {}", e))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(describe_token_error(provider, status.as_u16(), &body)));
    }

    let parsed: TokenResponse = serde_json::from_str(&body)
        .map_err(|e| anyhow!("Invalid token refresh response: {}", e))?;
    apply_token_response(data, parsed, Utc::now().timestamp());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oauth(client_secret: Option<&str>) -> CredentialData {
        CredentialData::OAuth {
            access_token: "old".into(),
            refresh_token: "refresh-1".into(),
            expires_at: 1_000,
            client_id: Some("client".into()),
            client_secret: client_secret.map(Into::into),
            tenant: None,
            scope: None,
        }
    }

    #[test]
    fn refresh_needed_when_expiring_or_empty() {
        let data = oauth(Some("s"));
        assert!(needs_refresh(&data, 1_000 - REFRESH_MARGIN_SECS + 1));
        assert!(!needs_refresh(&data, 1_000 - REFRESH_MARGIN_SECS - 1));

        let mut empty = oauth(Some("s"));
        if let CredentialData::OAuth { access_token, expires_at, .. } = &mut empty {
            access_token.clear();
            *expires_at = i64::MAX;
        }
        assert!(needs_refresh(&empty, 0));
    }

    #[test]
    fn google_needs_secret_microsoft_does_not() {
        assert!(can_refresh(OAuthProvider::Google, &oauth(Some("s"))));
        assert!(!can_refresh(OAuthProvider::Google, &oauth(None)));
        assert!(can_refresh(OAuthProvider::Microsoft, &oauth(None)));
        assert!(refresh_form(OAuthProvider::Google, &oauth(None)).is_err());
    }

    #[test]
    fn microsoft_form_has_scope_and_tenant_url() {
        let form = refresh_form(OAuthProvider::Microsoft, &oauth(None)).unwrap();
        assert!(form.contains(&("scope", DEFAULT_MICROSOFT_SCOPE.to_string())));
        assert!(!form.iter().any(|(k, _)| *k == "client_secret"));
        assert_eq!(
            token_url(OAuthProvider::Microsoft, &oauth(None)),
            "https://login.microsoftonline.com/common/oauth2/v2.0/token"
        );

        let mut custom = oauth(None);
        if let CredentialData::OAuth { tenant, .. } = &mut custom {
            *tenant = Some("consumers/../evil?x".into());
        }
        assert_eq!(
            token_url(OAuthProvider::Microsoft, &custom),
            "https://login.microsoftonline.com/consumers..evilx/oauth2/v2.0/token"
        );
    }

    #[test]
    fn token_response_updates_tokens_and_rotates_refresh_token() {
        let mut data = oauth(None);
        apply_token_response(
            &mut data,
            TokenResponse { access_token: "new".into(), expires_in: Some(3599), refresh_token: Some("refresh-2".into()) },
            10_000,
        );
        match data {
            CredentialData::OAuth { access_token, refresh_token, expires_at, .. } => {
                assert_eq!(access_token, "new");
                assert_eq!(refresh_token, "refresh-2");
                assert_eq!(expires_at, 13_599);
            }
            _ => unreachable!(),
        }

        let mut kept = oauth(Some("s"));
        apply_token_response(
            &mut kept,
            TokenResponse { access_token: "new".into(), expires_in: None, refresh_token: None },
            0,
        );
        assert!(matches!(kept, CredentialData::OAuth { ref refresh_token, .. } if refresh_token == "refresh-1"));
    }

    #[test]
    fn invalid_grant_error_explains_testing_status() {
        let msg = describe_token_error(
            OAuthProvider::Google,
            400,
            r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
        );
        assert!(msg.contains("invalid_grant"));
        assert!(msg.contains("7 days"));
    }
}
