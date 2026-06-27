use crate::providers::{Provider, ProviderError, ProviderResult, Report};
use chrono::Utc;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitStatus {
    Ok = 0,
    AnyFailed = 1,
    UsageError = 2,
    RenderError = 3,
}

pub struct RunOptions {
    pub debug: bool,
    /// True when the caller named a specific provider (e.g. `usage opencodego`).
    /// When false (default bulk run), AuthMissing is treated as a skip so that
    /// unconfigured optional providers do not raise exit code 1.
    pub explicit_request: bool,
}

pub fn run(
    requested: &[&str],
    all: &[Box<dyn Provider>],
    opts: RunOptions,
) -> (Report, ExitStatus) {
    let explicit_request = opts.explicit_request;
    let by_id: std::collections::HashMap<&str, &dyn Provider> =
        all.iter().map(|p| (p.id(), p.as_ref())).collect();

    let checked_at = Utc::now();

    let results: Arc<Mutex<BTreeMap<String, ProviderResult>>> =
        Arc::new(Mutex::new(BTreeMap::new()));
    let any_failed: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    // Deduplicate requested
    let mut seen = std::collections::HashSet::new();
    let unique: Vec<&str> = requested
        .iter()
        .filter(|&&id| seen.insert(id))
        .copied()
        .collect();

    std::thread::scope(|s| {
        for &id in &unique {
            if let Some(&provider) = by_id.get(id) {
                let results = Arc::clone(&results);
                let any_failed = Arc::clone(&any_failed);
                let id = id.to_string();

                s.spawn(move || {
                    let explicit_request = explicit_request;
                    let result = provider.fetch();

                    let pr = match result {
                        Ok(out) => ProviderResult {
                            limits: out.limits,
                            accounts: out.accounts,
                            error: None,
                        },
                        Err(e) => {
                            // In bulk (non-explicit) runs, AuthMissing means the
                            // provider is simply not configured — treat it as a skip
                            // so unconfigured optional providers don't raise exit 1.
                            // Explicit `usage <provider>` requests always set the flag.
                            let suppress = !explicit_request
                                && matches!(e, ProviderError::AuthMissing(_));
                            if !suppress {
                                *any_failed.lock().unwrap() = true;
                            }
                            ProviderResult {
                                limits: None,
                                accounts: vec![],
                                error: Some(e.to_string()),
                            }
                        }
                    };
                    results.lock().unwrap().insert(id, pr);
                });
            }
        }
    });

    let providers = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    let failed = *any_failed.lock().unwrap();

    let status = if failed { ExitStatus::AnyFailed } else { ExitStatus::Ok };

    (Report { checked_at, providers }, status)
}
