use crate::config::Config;
use crate::orchestrate::{run, ExitStatus, RunOptions};
use crate::providers::KNOWN_PROVIDER_IDS;
use crate::render::{bar::render_bar, json::render_json, text::render_text};
use std::io::{self, IsTerminal};

pub struct UsageArgs {
    pub provider: Option<String>,
    pub refresh: bool,
    pub bar: bool,
    pub human: bool,
    pub debug: bool,
}

fn should_use_color(is_terminal: bool) -> bool {
    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }
    if std::env::var("CLICOLOR").map(|v| v == "0").unwrap_or(false) {
        return false;
    }
    is_terminal
}

pub fn run_usage(args: UsageArgs) -> i32 {
    if let Some(ref p) = args.provider {
        if !KNOWN_PROVIDER_IDS.contains(&p.as_str()) {
            eprintln!(
                "usage {}: provider must be one of {}",
                p,
                KNOWN_PROVIDER_IDS.join(", ")
            );
            return ExitStatus::UsageError as i32;
        }
    }

    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("usage: {error}");
            return ExitStatus::UsageError as i32;
        }
    };

    let requested: Vec<&str> = match &args.provider {
        Some(p) => vec![p.as_str()],
        None => config.enabled_provider_ids(),
    };

    let providers = crate::cli::registry::real_providers(args.refresh, args.debug);

    let (report, status) = run(
        &requested,
        &providers,
        RunOptions {
            explicit_request: args.provider.is_some(),
        },
    );

    let stdout = io::stdout();
    let mut out = stdout.lock();

    let render_err = if args.bar {
        render_bar(
            &mut out,
            &report,
            &requested,
            should_use_color(io::stdout().is_terminal()),
        )
    } else if args.human {
        render_text(&mut out, &report, &requested)
    } else {
        render_json(&mut out, &report)
    };

    if let Err(e) = render_err {
        eprintln!("{}", e);
        return ExitStatus::RenderError as i32;
    }

    status as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn unset_all_color_envs() {
        std::env::remove_var("NO_COLOR");
        std::env::remove_var("CLICOLOR");
    }

    #[test]
    fn color_on_terminal_without_env_overrides() {
        let _guard = ENV_LOCK.lock().unwrap();
        unset_all_color_envs();
        assert!(should_use_color(true));
    }

    #[test]
    fn no_color_on_non_terminal() {
        let _guard = ENV_LOCK.lock().unwrap();
        unset_all_color_envs();
        assert!(!should_use_color(false));
    }

    #[test]
    fn no_color_env_disables_color_even_on_terminal() {
        let _guard = ENV_LOCK.lock().unwrap();
        unset_all_color_envs();
        std::env::set_var("NO_COLOR", "1");
        assert!(!should_use_color(true));
    }

    #[test]
    fn clicolor_zero_disables_color() {
        let _guard = ENV_LOCK.lock().unwrap();
        unset_all_color_envs();
        std::env::set_var("CLICOLOR", "0");
        assert!(!should_use_color(true));
    }

    #[test]
    fn clicolor_nonzero_allows_color_on_terminal() {
        let _guard = ENV_LOCK.lock().unwrap();
        unset_all_color_envs();
        std::env::set_var("CLICOLOR", "1");
        assert!(should_use_color(true));
    }
}
