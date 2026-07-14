use crate::accounts::Account;
use crate::cred::claude::ClaudeOAuth;
use serde_json::value::RawValue;

fn parse_raw(acct: &Account) -> Option<ClaudeOAuth> {
    crate::cred::claude::parse_claude_oauth(acct.raw_blob.get().as_bytes()).ok()
}

pub fn stored_access_token_owned(acct: &Account) -> String {
    parse_raw(acct)
        .and_then(|o| o.access_token)
        .unwrap_or_default()
}

pub fn stored_refresh_token(acct: &Account) -> String {
    parse_raw(acct)
        .and_then(|o| o.refresh_token)
        .unwrap_or_default()
}

pub fn stored_expires_at(acct: &Account) -> i64 {
    parse_raw(acct).and_then(|o| o.expires_at).unwrap_or(0)
}

pub fn rotate_raw_blob(
    raw_blob: &RawValue,
    tok: &super::refresh::Token,
) -> Result<Box<RawValue>, String> {
    let data = rotate_credential_raw(raw_blob.get().as_bytes(), tok)?;
    let raw = String::from_utf8(data).map_err(|e| e.to_string())?;
    RawValue::from_string(raw).map_err(|e| e.to_string())
}

pub fn rotate_credential_raw(raw: &[u8], tok: &super::refresh::Token) -> Result<Vec<u8>, String> {
    let mut m: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    let oauth = m
        .get_mut("claudeAiOauth")
        .and_then(|v| v.as_object_mut())
        .ok_or("rotateRawBlob: claudeAiOauth missing or wrong type")?;
    oauth.insert(
        "accessToken".into(),
        serde_json::Value::String(tok.access_token.clone()),
    );
    oauth.insert(
        "refreshToken".into(),
        serde_json::Value::String(tok.refresh_token.clone()),
    );
    oauth.insert(
        "expiresAt".into(),
        serde_json::Value::Number(tok.expires_at.into()),
    );
    serde_json::to_vec(&m).map_err(|e| e.to_string())
}
