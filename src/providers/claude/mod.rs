pub mod account;
pub mod profile;
pub mod reconcile;
pub mod refresh;

use super::{
    multiaccount::{
        budget_secs, recompute_reset_after, record_fetch_outcome, sort_account_results,
    },
    usagecache::Cache,
    AccountResult, Limit, Provider, ProviderError, ProviderOutput,
};
use crate::{
    accounts::{MemoryStore, Store},
    cred::{self, Credential},
    httpx::{DebugFn, Doer},
};
use account::{stored_access_token_owned, stored_expires_at, stored_refresh_token};
use chrono::Utc;
use reconcile::{reconcile, ReconcileInput};
use refresh::{RefreshClient, Token};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;

const ENDPOINT: &str = "https://api.anthropic.com/api/oauth/usage";
const BASE_TIMEOUT_SECS: u64 = 10;
const PER_ACCOUNT_BUDGET_SECS: u64 = 15;
const REFRESH_SKEW_MS: i64 = 30_000;

fn refresh_if_needed<F>(
    credential: Credential,
    now_ms: i64,
    exchange: F,
) -> Result<Credential, ProviderError>
where
    F: FnOnce(String) -> Result<Token, ProviderError>,
{
    let oauth = cred::claude::parse_claude_oauth(&credential.raw)
        .map_err(|e| ProviderError::Other(e.to_string()))?;
    let expires_at_ms = oauth.expires_at.unwrap_or(0);
    if expires_at_ms <= 0 || expires_at_ms >= now_ms + REFRESH_SKEW_MS {
        return Ok(credential);
    }

    let refresh_token = oauth
        .refresh_token
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            ProviderError::AuthDenied(
                "Claude access token expired and refresh token is missing — run `claude /login` to authenticate".into(),
            )
        })?;
    let token = exchange(refresh_token)?;
    let raw = account::rotate_credential_raw(&credential.raw, &token)
        .map_err(|e| ProviderError::Other(format!("could not refresh Claude credential: {e}")))?;

    Ok(Credential {
        access_token: token.access_token,
        raw,
    })
}

pub fn default_user_agent(version: &str) -> String {
    format!(
        "agent-limits/{} (claude; https://github.com/f4ah6o/agent-usage)",
        version
    )
}

pub struct ClaudeClient {
    doer: Arc<Doer>,
    refresh: RefreshClient,
    store: Arc<dyn Store>,
    cache: Cache,
    cache_bypass: bool,
}

impl ClaudeClient {
    pub fn new(
        user_agent: String,
        debug: Option<DebugFn>,
        store: Option<Arc<dyn Store>>,
        cache_bypass: bool,
    ) -> Self {
        let doer = Arc::new(Doer::new(
            user_agent.clone(),
            "claude",
            vec![("Anthropic-Beta".into(), "oauth-2025-04-20".into())],
            debug,
        ));
        let refresh = RefreshClient::new(Arc::clone(&doer));
        let store = store.unwrap_or_else(|| Arc::new(MemoryStore::new()));
        let cache = Cache::new("claude");
        Self {
            doer,
            refresh,
            store,
            cache,
            cache_bypass,
        }
    }

    fn read_live_credential(&self) -> Result<Option<Credential>, ProviderError> {
        match cred::claude::read_claude_credential() {
            Ok(c) => Ok(Some(c)),
            Err(cred::CredError::ClaudeNotFound) => Ok(None),
            Err(e) => Err(ProviderError::Other(e.to_string())),
        }
    }

    fn fetch_limits_fresh(
        &self,
        access_token: &str,
    ) -> Result<BTreeMap<String, Limit>, ProviderError> {
        #[derive(Deserialize)]
        struct Window {
            utilization: f64,
            resets_at: Option<String>,
        }

        let raw: BTreeMap<String, serde_json::Value> =
            self.doer
                .get(ENDPOINT, access_token, PER_ACCOUNT_BUDGET_SECS)?;

        let now = Utc::now();
        let mut limits = BTreeMap::new();

        for key in &["five_hour", "seven_day", "seven_day_sonnet"] {
            let Some(val) = raw.get(*key) else { continue };
            let win: Window = match serde_json::from_value(val.clone()) {
                Ok(w) => w,
                Err(_) => continue,
            };
            let Some(resets_str) = win.resets_at else {
                continue;
            };
            let resets = match resets_str.parse::<chrono::DateTime<Utc>>() {
                Ok(t) => t,
                Err(_) => {
                    return Err(ProviderError::Other(format!(
                        "claude window {} has unparseable resets_at {:?}",
                        key, resets_str
                    )))
                }
            };
            let secs = (resets - now).num_seconds().max(0);
            limits.insert(
                key.to_string(),
                Limit {
                    used_percent: win.utilization,
                    remaining_percent: 100.0 - win.utilization,
                    resets_at: resets,
                    reset_after_seconds: secs,
                },
            );
        }
        Ok(limits)
    }

    fn fetch_limits_cached(
        &self,
        access_token: &str,
        uuid: &str,
    ) -> Result<BTreeMap<String, Limit>, ProviderError> {
        if uuid.is_empty() {
            return self.fetch_limits_fresh(access_token);
        }
        if !self.cache_bypass {
            if let Some((cached, age)) = self.cache.get_with_age(uuid) {
                let refreshed = recompute_reset_after(cached, Utc::now());
                if let Some(ref dbg) = self.doer.debug {
                    dbg(&format!(
                        "[debug] claude: usage cache hit for {} (age {}s)\n",
                        uuid,
                        age.as_secs()
                    ));
                }
                return Ok(refreshed);
            }
        }
        let limits = self.fetch_limits_fresh(access_token)?;
        self.cache.put(uuid, &limits);
        Ok(limits)
    }

    fn refresh_error_message(e: &ProviderError) -> String {
        let msg = e.to_string();
        if msg.contains("invalid_grant") {
            "account credential expired (run `claude /login` to refresh)".into()
        } else if msg.contains("refresh endpoint") || msg.contains("broken") {
            format!(
                "agent-limits: claude: refresh endpoint rejected request ({}); this is likely an agent-limits refresh implementation issue. Run 'claude /login' to work around it.",
                msg
            )
        } else {
            msg
        }
    }

    fn map_refresh_error(e: ProviderError) -> ProviderError {
        let msg = Self::refresh_error_message(&e);
        match e {
            ProviderError::AuthMissing(_) => ProviderError::AuthMissing(msg),
            ProviderError::AuthDenied(_) => ProviderError::AuthDenied(msg),
            ProviderError::Transient(_) => ProviderError::Transient(msg),
            ProviderError::Other(_) => ProviderError::Other(msg),
        }
    }

    fn refresh_live_credential(&self, credential: Credential) -> Result<Credential, ProviderError> {
        refresh_if_needed(credential, Utc::now().timestamp_millis(), |refresh_token| {
            self.refresh
                .exchange(refresh_token)
                .map_err(Self::map_refresh_error)
        })
    }
}

impl Provider for ClaudeClient {
    fn id(&self) -> &str {
        "claude"
    }

    fn fetch(&self) -> Result<ProviderOutput, ProviderError> {
        let live = self
            .read_live_credential()?
            .map(|credential| self.refresh_live_credential(credential))
            .transpose()?;

        let stored = self.store.list().unwrap_or_else(|e| {
            eprintln!("agent-limits: claude: could not read account store ({}); proceeding with live credential only", e);
            vec![]
        });

        let profile_doer = Arc::clone(&self.doer);
        let reconcile_out = reconcile(ReconcileInput {
            live_blob: live.as_ref(),
            stored: &stored,
            lookup_profile: &|token| profile::get_profile(&profile_doer, token),
            now: Utc::now(),
        });

        // Persist inserted/upserted accounts
        if reconcile_out.inserted || reconcile_out.upserted {
            for acct in &reconcile_out.accounts {
                if acct.uuid == reconcile_out.active_uuid {
                    let _ = self.store.upsert(acct.clone());
                    break;
                }
            }
        }

        if live.is_none() && reconcile_out.accounts.is_empty() {
            return Err(ProviderError::AuthMissing(
                "claude token not found — run `claude /login` to authenticate".into(),
            ));
        }

        if let Some(ref warn) = reconcile_out.capture_warn {
            eprintln!("{}", warn);
        }

        let total_accounts = reconcile_out.accounts.len()
            + if reconcile_out.live_unstored.is_some() {
                1
            } else {
                0
            };
        let _budget_secs = budget_secs(BASE_TIMEOUT_SECS, PER_ACCOUNT_BUDGET_SECS, total_accounts);

        let mut account_results: Vec<AccountResult> = vec![];
        let mut transient_count = 0usize;
        let mut success_count = 0usize;

        // Synthetic live-unstored row
        if let Some(ref live_cred) = reconcile_out.live_unstored {
            let limits_result = self.fetch_limits_fresh(&live_cred.access_token);
            let mut ar = AccountResult {
                email: "(live Claude account)".into(),
                plan: String::new(),
                active: true,
                limits: None,
                error: None,
            };
            let (ok, trans) = record_fetch_outcome(&mut ar, limits_result);
            if ok {
                success_count += 1;
            }
            if trans {
                transient_count += 1;
            }
            account_results.push(ar);
        }

        // Per-account sequential fetch
        let now = Utc::now();
        for acct in &reconcile_out.accounts {
            let mut ar = AccountResult {
                email: acct.email.clone(),
                plan: acct.rate_limit_tier.clone(),
                active: acct.uuid == reconcile_out.active_uuid,
                limits: None,
                error: None,
            };

            // Refresh if near expiry
            let mut access_token = stored_access_token_owned(acct);
            let expires_at_ms = stored_expires_at(acct);
            if expires_at_ms > 0 {
                let now_plus_skew = now.timestamp_millis() + REFRESH_SKEW_MS;
                if expires_at_ms < now_plus_skew {
                    let refresh_token = stored_refresh_token(acct);
                    if refresh_token.is_empty() {
                        let e = ProviderError::AuthDenied(
                            "Claude account is missing a refresh token — run `claude /login` to authenticate".into(),
                        );
                        ar.error = Some(e.to_string());
                        account_results.push(ar);
                        continue;
                    }
                    match self.refresh.exchange(refresh_token) {
                        Err(e) => {
                            let e = Self::map_refresh_error(e);
                            ar.error = Some(e.to_string());
                            if e.is_transient() {
                                transient_count += 1;
                            }
                            account_results.push(ar);
                            continue;
                        }
                        Ok(tok) => {
                            access_token = tok.access_token.clone();
                            if let Ok(new_blob) = account::rotate_raw_blob(&acct.raw_blob, &tok) {
                                let mut updated = acct.clone();
                                updated.raw_blob = new_blob;
                                let _ = self.store.upsert(updated);
                            }
                        }
                    }
                }
            }

            let limits_result = self.fetch_limits_cached(&access_token, &acct.uuid);
            let (ok, trans) = record_fetch_outcome(&mut ar, limits_result);
            if ok {
                success_count += 1;
            }
            if trans {
                transient_count += 1;
            }
            account_results.push(ar);
        }

        sort_account_results(&mut account_results);

        if success_count == 0 && transient_count > 0 {
            return Err(ProviderError::Transient(format!(
                "all {} account fetch(es) failed",
                account_results.len()
            )));
        }

        Ok(ProviderOutput {
            limits: None,
            accounts: account_results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::refresh_if_needed;
    use crate::cred::Credential;
    use crate::providers::claude::refresh::Token;
    use crate::providers::ProviderError;

    fn credential(expires_at: i64) -> Credential {
        Credential {
            access_token: "old-access".into(),
            raw: format!(
                r#"{{"claudeAiOauth":{{"accessToken":"old-access","refreshToken":"refresh","expiresAt":{expires_at}}}}}"#
            )
            .into_bytes(),
        }
    }

    #[test]
    fn does_not_refresh_a_credential_with_enough_lifetime() {
        let credential = credential(2_000_000);
        let result = refresh_if_needed(credential, 1_000_000, |_| {
            panic!("an unexpired credential must not be refreshed")
        })
        .expect("credential remains usable");

        assert_eq!(result.access_token, "old-access");
    }

    #[test]
    fn refreshes_expired_credential_and_rotates_in_memory_blob() {
        let result = refresh_if_needed(credential(999_999), 1_000_000, |refresh_token| {
            assert_eq!(refresh_token, "refresh");
            Ok(Token {
                access_token: "new-access".into(),
                refresh_token: "new-refresh".into(),
                expires_at: 2_000_000,
            })
        })
        .expect("refresh succeeds");

        assert_eq!(result.access_token, "new-access");
        let oauth = crate::cred::claude::parse_claude_oauth(&result.raw).unwrap();
        assert_eq!(oauth.access_token.as_deref(), Some("new-access"));
        assert_eq!(oauth.refresh_token.as_deref(), Some("new-refresh"));
        assert_eq!(oauth.expires_at, Some(2_000_000));
    }

    #[test]
    fn expired_credential_without_refresh_token_is_auth_error() {
        let credential = Credential {
            access_token: "old-access".into(),
            raw: br#"{"claudeAiOauth":{"accessToken":"old-access","expiresAt":999999}}"#.to_vec(),
        };
        let error = refresh_if_needed(credential, 1_000_000, |_| {
            Err(ProviderError::Other("must not be called".into()))
        })
        .expect_err("missing refresh token must fail");

        assert!(matches!(error, ProviderError::AuthDenied(_)));
        assert!(!error.to_string().contains("old-access"));
    }
}
