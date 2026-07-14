use super::{CredError, Credential};
use serde::Deserialize;

#[derive(Deserialize)]
struct ClaudeCred {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: ClaudeOAuth,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ClaudeOAuth {
    #[serde(rename = "accessToken")]
    pub(crate) access_token: Option<String>,
    #[serde(rename = "refreshToken")]
    pub(crate) refresh_token: Option<String>,
    #[serde(rename = "expiresAt")]
    pub(crate) expires_at: Option<i64>,
}

pub(crate) fn parse_claude_oauth(data: &[u8]) -> Result<ClaudeOAuth, CredError> {
    let raw: ClaudeCred = serde_json::from_slice(data)
        .map_err(|e| CredError::Other(format!("claude credential is not valid JSON: {e}")))?;
    Ok(raw.claude_ai_oauth)
}

pub fn parse_claude_cred(data: &[u8]) -> Result<Credential, CredError> {
    let oauth = parse_claude_oauth(data)?;
    let access_token = oauth
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or(CredError::ClaudeNotFound)?;
    Ok(Credential {
        access_token,
        raw: data.to_vec(),
    })
}

pub fn read_claude_credential() -> Result<Credential, CredError> {
    #[cfg(target_os = "macos")]
    {
        super::keychain_darwin::read_claude_credential()
    }
    #[cfg(target_os = "linux")]
    {
        super::keychain_linux::read_claude_credential()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(CredError::Other(
            "reading Claude credential not supported on this platform".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_claude_oauth;

    #[test]
    fn parses_refresh_metadata_without_needing_the_token_value() {
        let oauth = parse_claude_oauth(
            br#"{
                "claudeAiOauth": {
                    "accessToken": "access",
                    "refreshToken": "refresh",
                    "expiresAt": 12345
                }
            }"#,
        )
        .expect("valid Claude credential");

        assert_eq!(oauth.access_token.as_deref(), Some("access"));
        assert_eq!(oauth.refresh_token.as_deref(), Some("refresh"));
        assert_eq!(oauth.expires_at, Some(12345));
    }
}
