use super::{CredError, Credential};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct CodexAuth {
    tokens: CodexTokens,
}

#[derive(Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    id_token: Option<String>,
}

fn codex_auth_path() -> Result<PathBuf, CredError> {
    let home = dirs::home_dir().ok_or_else(|| {
        CredError::Other(format!(
            "{}: cannot resolve home directory",
            CredError::CodexNotFound
        ))
    })?;
    Ok(home.join(".codex").join("auth.json"))
}

pub fn parse_codex_cred(data: &[u8]) -> Result<Credential, CredError> {
    let raw: CodexAuth = serde_json::from_slice(data)
        .map_err(|e| CredError::Other(format!("codex auth.json is not valid JSON: {e}")))?;
    let access_token = raw
        .tokens
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or(CredError::CodexNotFound)?;
    Ok(Credential {
        access_token,
        raw: data.to_vec(),
    })
}

pub fn read_codex_credential() -> Result<Credential, CredError> {
    let path = codex_auth_path()?;
    let data = std::fs::read(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CredError::CodexNotFound
        } else {
            CredError::Other(format!("reading codex auth.json: {e}"))
        }
    })?;
    parse_codex_cred(&data)
}

/// Extract the id_token string from the raw codex auth.json blob.
pub fn extract_id_token(raw: &[u8]) -> Option<String> {
    let auth: CodexAuth = serde_json::from_slice(raw).ok()?;
    auth.tokens.id_token.filter(|s| !s.is_empty())
}
