#![cfg(target_os = "macos")]

use super::{claude::parse_claude_cred, CredError, Credential};
use std::process::Command;

const CLAUDE_SERVICE: &str = "Claude Code-credentials";

fn run_security(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("security")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        // exit code 44 = not found
        if out.status.code() == Some(44) {
            return Err("not found (44)".into());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(stderr.into_owned());
    }
    Ok(out.stdout)
}

pub fn read_claude_credential() -> Result<Credential, CredError> {
    let out =
        run_security(&["find-generic-password", "-s", CLAUDE_SERVICE, "-w"]).map_err(|e| {
            if e.contains("44") || e.contains("not found") || e.contains("could not be found") {
                CredError::ClaudeNotFound
            } else {
                CredError::Other(format!("reading Claude keychain item: {e}"))
            }
        })?;
    let data = out.strip_suffix(b"\n").unwrap_or(&out);
    parse_claude_cred(data)
}
