use super::{Limit, Provider, ProviderError, ProviderOutput};
use crate::httpx::DebugFn;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const DEFAULT_API_SERVER: &str = "https://server.codeium.com";
const USER_STATUS_PATH: &str = "/exa.seat_management_pb.SeatManagementService/GetUserStatus";
const FETCH_TIMEOUT_SECS: u64 = 15;

pub fn default_user_agent(version: &str) -> String {
    format!(
        "agent-limits/{} (devin; https://github.com/f4ah6o/agent-limits)",
        version
    )
}

struct Credentials {
    api_key: String,
    api_server: String,
}

pub struct DevinClient {
    user_agent: String,
    debug: Option<DebugFn>,
}

impl DevinClient {
    pub fn new(user_agent: String, debug: Option<DebugFn>) -> Self {
        Self { user_agent, debug }
    }

    fn log(&self, msg: &str) {
        if let Some(debug) = &self.debug {
            debug(&format!("[debug] devin: {}\n", msg));
        }
    }

    fn fetch_fresh(&self) -> Result<BTreeMap<String, Limit>, ProviderError> {
        let credentials = read_credentials()?;
        let version = devin_version();
        let url = format!("{}{}", credentials.api_server, USER_STATUS_PATH);
        self.log(&format!("POST {}", url));

        let body = serde_json::json!({
            "metadata": {
                "apiKey": credentials.api_key,
                "ideName": "devin",
                "ideVersion": &version,
                "extensionVersion": &version,
                "locale": "en"
            }
        });

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(FETCH_TIMEOUT_SECS)))
            .build()
            .new_agent();

        let response = agent
            .post(&url)
            .header("User-Agent", &self.user_agent)
            .header("Connect-Protocol-Version", "1")
            .send_json(&body);

        let response = match response {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(code)) => {
                self.log(&format!("HTTP {}", code));
                return match code {
                    401 | 403 => Err(ProviderError::AuthDenied(
                        "Devin local API key is no longer accepted — run `devin auth login`".into(),
                    )),
                    429 | 500..=599 => Err(ProviderError::Transient(format!(
                        "Devin user-status API returned HTTP {}",
                        code
                    ))),
                    _ => Err(ProviderError::Other(format!(
                        "Devin user-status API returned HTTP {}",
                        code
                    ))),
                };
            }
            Err(error) => {
                self.log(&format!("request error: {}", error));
                return Err(ProviderError::Transient(format!(
                    "Devin user-status API is unavailable: {}",
                    error
                )));
            }
        };

        self.log(&format!("HTTP {} ok", response.status()));
        let payload = response.into_body().read_to_string().map_err(|error| {
            ProviderError::Transient(format!(
                "Devin user-status API response could not be read: {}",
                error
            ))
        })?;

        parse_usage(&payload, Utc::now())
    }
}

impl Provider for DevinClient {
    fn id(&self) -> &str {
        "devin"
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn fetch(&self) -> Result<ProviderOutput, ProviderError> {
        self.fetch_fresh().map(|limits| ProviderOutput {
            limits: Some(limits),
            accounts: vec![],
        })
    }
}

fn credentials_path() -> Result<PathBuf, ProviderError> {
    if let Some(path) = std::env::var_os("DEVIN_CREDENTIALS_FILE") {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        if !data_home.is_empty() {
            return Ok(PathBuf::from(data_home)
                .join("devin")
                .join("credentials.toml"));
        }
    }

    let home = dirs::home_dir().ok_or_else(|| {
        ProviderError::AuthMissing(
            "could not determine the home directory for Devin CLI credentials".into(),
        )
    })?;
    Ok(home
        .join(".local")
        .join("share")
        .join("devin")
        .join("credentials.toml"))
}

fn read_credentials() -> Result<Credentials, ProviderError> {
    let path = credentials_path()?;
    let contents = fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProviderError::AuthMissing(format!(
                "Devin CLI credentials not found at {} — run `devin auth login`",
                path.display()
            ))
        } else {
            ProviderError::Other(format!(
                "could not read Devin CLI credentials at {}: {}",
                path.display(),
                error
            ))
        }
    })?;

    let api_key = parse_toml_string(&contents, "windsurf_api_key")
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ProviderError::AuthMissing(format!(
                "Devin CLI credentials at {} do not contain windsurf_api_key — run `devin auth login`",
                path.display()
            ))
        })?;

    let api_server = parse_toml_string(&contents, "api_server_url")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_API_SERVER.to_string());
    let api_server = api_server.trim().trim_end_matches('/').to_string();

    if !api_server.starts_with("https://") || api_server.chars().any(char::is_whitespace) {
        return Err(ProviderError::Other(
            "Devin CLI api_server_url must be a valid HTTPS URL".into(),
        ));
    }

    Ok(Credentials {
        api_key,
        api_server,
    })
}

fn parse_toml_string(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, raw_value)) = line.split_once('=') else {
            continue;
        };
        if name.trim() != key {
            continue;
        }
        return parse_quoted_toml_value(raw_value.trim());
    }
    None
}

fn parse_quoted_toml_value(value: &str) -> Option<String> {
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    let trailing = rest[end + quote.len_utf8()..].trim();
    if !trailing.is_empty() && !trailing.starts_with('#') {
        return None;
    }
    Some(rest[..end].to_string())
}

fn devin_version() -> String {
    let output = Command::new("devin").arg("version").output();
    let Ok(output) = output else {
        return "unknown".into();
    };
    if !output.status.success() {
        return "unknown".into();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .find(|token| {
            let token = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
            token.contains('.')
                && token
                    .split('.')
                    .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        })
        .map(|token| {
            token
                .trim_matches(|c: char| !c.is_ascii_digit() && c != '.')
                .to_string()
        })
        .unwrap_or_else(|| "unknown".into())
}

fn parse_usage(
    payload: &str,
    now: DateTime<Utc>,
) -> Result<BTreeMap<String, Limit>, ProviderError> {
    let payload: Value = serde_json::from_str(payload).map_err(|error| {
        ProviderError::Other(format!(
            "Devin user-status API returned invalid JSON: {}",
            error
        ))
    })?;

    let plan_status = payload
        .get("userStatus")
        .and_then(|value| value.get("planStatus"))
        .ok_or_else(|| {
            ProviderError::Other(
                "Devin user-status response did not contain userStatus.planStatus".into(),
            )
        })?;

    let mut limits = BTreeMap::new();
    add_quota(
        &mut limits,
        "daily",
        plan_status,
        "dailyQuotaRemainingPercent",
        "dailyQuotaResetAtUnix",
        now,
    );
    add_quota(
        &mut limits,
        "weekly",
        plan_status,
        "weeklyQuotaRemainingPercent",
        "weeklyQuotaResetAtUnix",
        now,
    );

    if limits.is_empty() {
        return Err(ProviderError::Other(
            "Devin user-status response did not contain daily or weekly quota data".into(),
        ));
    }

    Ok(limits)
}

fn add_quota(
    limits: &mut BTreeMap<String, Limit>,
    key: &str,
    plan_status: &Value,
    remaining_key: &str,
    reset_key: &str,
    now: DateTime<Utc>,
) {
    let Some(remaining) = number(plan_status.get(remaining_key)) else {
        return;
    };
    let Some(reset_unix) = number(plan_status.get(reset_key)) else {
        return;
    };
    if !remaining.is_finite() || !reset_unix.is_finite() {
        return;
    }

    let remaining = remaining.clamp(0.0, 100.0);
    let Some(resets_at) = DateTime::<Utc>::from_timestamp(reset_unix.floor() as i64, 0) else {
        return;
    };
    let reset_after_seconds = (resets_at - now).num_seconds().max(0);

    limits.insert(
        key.to_string(),
        Limit {
            used_percent: 100.0 - remaining,
            remaining_percent: remaining,
            resets_at,
            reset_after_seconds,
        },
    );
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_devin_cli_credentials() {
        let contents = r#"
            windsurf_api_key = "devin-session-token$example"
            api_server_url = "https://server.codeium.com"
        "#;
        assert_eq!(
            parse_toml_string(contents, "windsurf_api_key").as_deref(),
            Some("devin-session-token$example")
        );
        assert_eq!(
            parse_toml_string(contents, "api_server_url").as_deref(),
            Some("https://server.codeium.com")
        );
    }

    #[test]
    fn parses_single_quoted_toml_values_and_comments() {
        let contents = "api_server_url = 'https://server.codeium.com' # managed by Devin\n";
        assert_eq!(
            parse_toml_string(contents, "api_server_url").as_deref(),
            Some("https://server.codeium.com")
        );
    }

    #[test]
    fn maps_remaining_quota_to_used_limits() {
        let now = DateTime::<Utc>::from_timestamp(1_760_000_000, 0).unwrap();
        let payload = r#"{
            "userStatus": {
                "planStatus": {
                    "dailyQuotaRemainingPercent": 75,
                    "dailyQuotaResetAtUnix": 1760010000,
                    "weeklyQuotaRemainingPercent": "40",
                    "weeklyQuotaResetAtUnix": "1760050000"
                }
            }
        }"#;

        let limits = parse_usage(payload, now).unwrap();
        let daily = limits.get("daily").unwrap();
        assert_eq!(daily.used_percent, 25.0);
        assert_eq!(daily.remaining_percent, 75.0);
        assert_eq!(daily.reset_after_seconds, 10_000);

        let weekly = limits.get("weekly").unwrap();
        assert_eq!(weekly.used_percent, 60.0);
        assert_eq!(weekly.remaining_percent, 40.0);
        assert_eq!(weekly.reset_after_seconds, 50_000);
    }

    #[test]
    fn rejects_response_without_quota_data() {
        let now = Utc::now();
        let error = parse_usage(r#"{"userStatus":{"planStatus":{}}}"#, now).unwrap_err();
        assert!(error.to_string().contains("quota data"));
    }
}
