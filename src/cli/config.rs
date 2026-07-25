use crate::config::{config_path, Config};
use crate::providers::KNOWN_PROVIDER_IDS;

pub enum ConfigAction {
    Enable { provider: String },
    Disable { provider: String },
    List,
}

pub fn run_config(action: ConfigAction) -> i32 {
    match action {
        ConfigAction::Enable { provider } => set_provider(provider, true),
        ConfigAction::Disable { provider } => set_provider(provider, false),
        ConfigAction::List => list_providers(),
    }
}

fn set_provider(provider: String, enabled: bool) -> i32 {
    if !KNOWN_PROVIDER_IDS.contains(&provider.as_str()) {
        eprintln!(
            "config: provider must be one of {}",
            KNOWN_PROVIDER_IDS.join(", ")
        );
        return 2;
    }

    let mut config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("config: {error}");
            return 1;
        }
    };
    config.set_enabled(&provider, enabled);
    if let Err(error) = config.save() {
        eprintln!("config: {error}");
        return 1;
    }

    println!(
        "{provider}: {}",
        if enabled { "enabled" } else { "disabled" }
    );
    0
}

fn list_providers() -> i32 {
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("config: {error}");
            return 1;
        }
    };

    for provider in KNOWN_PROVIDER_IDS {
        println!(
            "{provider}: {}",
            if config.is_enabled(provider) {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
    println!("config: {}", config_path().display());
    0
}
