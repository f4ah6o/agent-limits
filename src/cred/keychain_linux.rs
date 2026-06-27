#![cfg(target_os = "linux")]

use super::{claude::parse_claude_cred, CredError, Credential};
use std::path::PathBuf;

fn cred_path() -> Result<PathBuf, CredError> {
    let home =
        dirs::home_dir().ok_or_else(|| CredError::Other("cannot resolve home directory".into()))?;
    Ok(home.join(".claude").join(".credentials.json"))
}

pub fn read_claude_credential() -> Result<Credential, CredError> {
    let path = cred_path()?;
    let data = std::fs::read(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CredError::ClaudeNotFound
        } else {
            CredError::Other(format!("reading Claude credentials: {e}"))
        }
    })?;
    parse_claude_cred(&data)
}
