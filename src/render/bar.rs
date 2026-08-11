use crate::providers::{provider_title, AccountResult, Limit, Report};
use crate::render::text::{format_reset_duration, rate_limit_tier_label, text_labels};
use std::collections::BTreeMap;
use std::io::Write;

const BAR_WIDTH: usize = 30;
const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const GRAY: &str = "\x1b[90m";

fn color_for_pct(pct: f64) -> &'static str {
    if pct < 30.0 {
        GREEN
    } else if pct <= 70.0 {
        YELLOW
    } else {
        RED
    }
}

pub fn render_bar<W: Write>(
    w: &mut W,
    report: &Report,
    requested: &[&str],
) -> std::io::Result<()> {
    let use_color = std::env::var("NO_COLOR").is_err()
        && std::env::var("CLICOLOR").map(|v| v != "0").unwrap_or(true);

    let mut sections: Vec<String> = vec![];
    for &id in requested {
        let result = match report.providers.get(id) {
            Some(r) => r,
            None => continue,
        };
        let title = format!("{} usage", provider_title(id));

        if !result.accounts.is_empty() {
            let mut lines = vec![title];
            render_accounts_bar(&mut lines, id, &result.accounts, use_color);
            sections.push(lines.join("\n"));
            continue;
        }

        if let Some(ref err) = result.error {
            if !err.is_empty() {
                let msg = if use_color {
                    format!("{}{}:{} {}", RED, title, RESET, err)
                } else {
                    format!("{}: {}", title, err)
                };
                sections.push(msg);
                continue;
            }
        }

        let limits = match result.limits.as_ref() {
            Some(l) if !l.is_empty() => l,
            _ => continue,
        };

        let mut lines = vec![title];
        render_limits_bar(&mut lines, id, limits, use_color);
        sections.push(lines.join("\n"));
    }

    if sections.is_empty() {
        return Ok(());
    }
    writeln!(w, "{}", sections.join("\n\n"))
}

fn render_limits_bar(
    lines: &mut Vec<String>,
    provider_id: &str,
    limits: &BTreeMap<String, Limit>,
    use_color: bool,
) {
    let known = text_labels(provider_id);
    let mut seen = std::collections::HashSet::new();
    let mut items: Vec<(String, &Limit)> = vec![];

    for entry in known {
        if let Some(limit) = limits.get(entry.key) {
            seen.insert(entry.key);
            items.push((entry.label.to_string(), limit));
        }
    }

    let mut unknown: Vec<&String> = limits
        .keys()
        .filter(|k| !seen.contains(k.as_str()))
        .collect();
    unknown.sort();
    for k in unknown {
        items.push((k.clone(), &limits[k]));
    }

    let label_width = items
        .iter()
        .map(|(l, _)| l.len())
        .max()
        .unwrap_or(6)
        .max(6);

    for (label, limit) in &items {
        lines.push(format_bar_line(label, label_width, limit, use_color));
    }
}

fn render_accounts_bar(
    lines: &mut Vec<String>,
    provider_id: &str,
    accts: &[AccountResult],
    use_color: bool,
) {
    for ar in accts {
        let mut header = format!("  {}", ar.email);
        if ar.active {
            header.push_str(" (active)");
        }
        let plan_label = rate_limit_tier_label(&ar.plan);
        if !plan_label.is_empty() {
            header.push_str(&format!(" [{}]", plan_label));
        }
        if let Some(ref err) = ar.error {
            if !err.is_empty() {
                lines.push(format!("{}: {}", header, err));
                continue;
            }
        }
        lines.push(header);
        if let Some(ref lims) = ar.limits {
            render_limits_bar(lines, provider_id, lims, use_color);
        }
    }
}

fn format_bar_line(label: &str, label_width: usize, l: &Limit, use_color: bool) -> String {
    let filled = (l.used_percent / 100.0 * BAR_WIDTH as f64).round() as usize;
    let filled = filled.min(BAR_WIDTH);
    let unfilled = BAR_WIDTH - filled;

    let bar = if use_color {
        let fill_color = color_for_pct(l.used_percent);
        format!(
            "{}{}{}{}",
            fill_color,
            "█".repeat(filled),
            GRAY,
            "░".repeat(unfilled),
        )
    } else {
        format!("{}{}", "█".repeat(filled), "░".repeat(unfilled))
    };

    let pct_str = format!("{:>5.1}%", l.used_percent);
    let reset_str = format_reset_duration(l.reset_after_seconds);

    if use_color {
        let pct_color = color_for_pct(l.used_percent);
        format!(
            "  {:<width$}  {}{}  {}{}{}  {}",
            label,
            bar, RESET,
            pct_color, pct_str, RESET,
            reset_str,
            width = label_width,
        )
    } else {
        format!(
            "  {:<width$}  {}  {}  {}",
            label,
            bar,
            pct_str,
            reset_str,
            width = label_width,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::ProviderResult;
    use chrono::Utc;

    fn make_limit(used: f64, remaining: f64, reset_sec: i64) -> Limit {
        Limit {
            used_percent: used,
            remaining_percent: remaining,
            resets_at: Utc::now() + chrono::Duration::seconds(reset_sec),
            reset_after_seconds: reset_sec,
        }
    }

    fn make_report(limits: BTreeMap<String, Limit>) -> Report {
        let mut providers = BTreeMap::new();
        providers.insert(
            "opencodego".to_string(),
            ProviderResult {
                limits: Some(limits),
                accounts: vec![],
                error: None,
            },
        );
        Report {
            checked_at: Utc::now(),
            providers,
        }
    }

    fn make_bar_line_count(line: &str) -> (usize, usize) {
        let filled = line.matches("█").count();
        let unfilled = line.matches("░").count();
        (filled, unfilled)
    }

    #[test]
    fn renders_bar_for_opencodego_limits() {
        let mut limits = BTreeMap::new();
        limits.insert("five_hour".to_string(), make_limit(80.0, 20.0, 7200));
        limits.insert("seven_day".to_string(), make_limit(50.0, 50.0, 172800));
        limits.insert("monthly".to_string(), make_limit(25.0, 75.0, 1296000));

        let report = make_report(limits);
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Opencodego usage"));
        assert!(output.contains("5-hour"));
        assert!(output.contains("7-day"));
        assert!(output.contains("Monthly"));
        assert!(output.contains("80.0%"));
        assert!(output.contains("50.0%"));
        assert!(output.contains("25.0%"));
        assert!(output.contains("█"));
        assert!(output.contains("░"));
    }

    #[test]
    fn bar_filled_proportion_matches_used_percent() {
        let mut limits = BTreeMap::new();
        limits.insert("five_hour".to_string(), make_limit(50.0, 50.0, 3600));

        let report = make_report(limits);
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let line = output
            .lines()
            .find(|l| l.contains("5-hour"))
            .expect("bar line for 5-hour present");
        let (filled, unfilled) = make_bar_line_count(line);
        assert_eq!(filled, 15);
        assert_eq!(unfilled, 15);
    }

    #[test]
    fn renders_error_when_present() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "opencodego".to_string(),
            ProviderResult {
                limits: None,
                accounts: vec![],
                error: Some("auth denied".to_string()),
            },
        );
        let report = Report {
            checked_at: Utc::now(),
            providers,
        };
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Opencodego usage"));
        assert!(output.contains("auth denied"));
    }

    #[test]
    fn empty_when_no_providers() {
        let report = Report {
            checked_at: Utc::now(),
            providers: BTreeMap::new(),
        };
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn bar_zero_percent_is_all_empty() {
        let mut limits = BTreeMap::new();
        limits.insert("five_hour".to_string(), make_limit(0.0, 100.0, 3600));

        let report = make_report(limits);
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let line = output
            .lines()
            .find(|l| l.contains("5-hour"))
            .expect("bar line for 5-hour present");
        let (filled, unfilled) = make_bar_line_count(line);
        assert_eq!(filled, 0);
        assert_eq!(unfilled, BAR_WIDTH);
    }

    #[test]
    fn bar_full_percent_is_all_filled() {
        let mut limits = BTreeMap::new();
        limits.insert("five_hour".to_string(), make_limit(100.0, 0.0, 0));

        let report = make_report(limits);
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let line = output
            .lines()
            .find(|l| l.contains("5-hour"))
            .expect("bar line for 5-hour present");
        let (filled, unfilled) = make_bar_line_count(line);
        assert_eq!(filled, BAR_WIDTH);
        assert_eq!(unfilled, 0);
    }

    #[test]
    fn bar_width_is_constant() {
        let mut limits = BTreeMap::new();
        limits.insert("five_hour".to_string(), make_limit(33.3, 66.7, 3600));

        let report = make_report(limits);
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let line = output
            .lines()
            .find(|l| l.contains("5-hour"))
            .expect("bar line for 5-hour present");
        let (filled, unfilled) = make_bar_line_count(line);
        assert_eq!(filled + unfilled, BAR_WIDTH);
    }

    #[test]
    fn handles_unknown_limit_keys() {
        let mut limits = BTreeMap::new();
        limits.insert("five_hour".to_string(), make_limit(50.0, 50.0, 3600));
        limits.insert("custom_window".to_string(), make_limit(20.0, 80.0, 5400));

        let report = make_report(limits);
        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("5-hour"));
        assert!(output.contains("custom_window"));
    }

    #[test]
    fn format_bar_line_no_color_has_no_escape_codes() {
        let limit = make_limit(80.0, 20.0, 7200);
        let line = format_bar_line("5-hour", 12, &limit, false);

        assert!(!line.contains("\x1b["));
        assert!(line.contains("█"));
        assert!(line.contains("░"));
    }

    #[test]
    fn format_bar_line_with_color_has_escape_codes() {
        let limit = make_limit(80.0, 20.0, 7200);
        let line = format_bar_line("5-hour", 12, &limit, true);

        assert!(line.contains("\x1b[31m")); // red for >70%
        assert!(line.contains(RESET));
    }

    #[test]
    fn color_boundary_70_percent_is_yellow() {
        let limit = make_limit(70.0, 30.0, 3600);
        let line = format_bar_line("5-hour", 12, &limit, true);

        assert!(line.contains("\x1b[33m")); // yellow (not red)
        assert!(!line.contains("\x1b[31m"));
    }

    #[test]
    fn color_boundary_30_percent_is_yellow() {
        let limit = make_limit(30.0, 70.0, 3600);
        let line = format_bar_line("5-hour", 12, &limit, true);

        assert!(line.contains("\x1b[33m")); // yellow
        assert!(!line.contains("\x1b[32m"));
    }

    #[test]
    fn multiple_providers_rendered() {
        let mut limits1 = BTreeMap::new();
        limits1.insert("five_hour".to_string(), make_limit(80.0, 20.0, 7200));
        let mut limits2 = BTreeMap::new();
        limits2.insert("five_hour".to_string(), make_limit(50.0, 50.0, 3600));

        let mut providers = BTreeMap::new();
        providers.insert(
            "opencodego".to_string(),
            ProviderResult {
                limits: Some(limits1),
                accounts: vec![],
                error: None,
            },
        );
        providers.insert(
            "claude".to_string(),
            ProviderResult {
                limits: Some(limits2),
                accounts: vec![],
                error: None,
            },
        );

        let report = Report {
            checked_at: Utc::now(),
            providers,
        };

        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego", "claude"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("Opencodego usage"));
        assert!(output.contains("Claude usage"));
        assert!(output.contains("80.0%"));
        assert!(output.contains("50.0%"));
    }

    #[test]
    fn accounts_rendered_with_bars() {
        let mut limits = BTreeMap::new();
        limits.insert("five_hour".to_string(), make_limit(60.0, 40.0, 3600));

        let mut providers = BTreeMap::new();
        providers.insert("opencodego".to_string(), ProviderResult {
            limits: None,
            accounts: vec![AccountResult {
                email: "test@example.com".to_string(),
                plan: "pro".to_string(),
                active: true,
                limits: Some(limits),
                error: None,
            }],
            error: None,
        });

        let report = Report {
            checked_at: Utc::now(),
            providers,
        };

        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("test@example.com"));
        assert!(output.contains("(active)"));
        assert!(output.contains("[pro]"));
        assert!(output.contains("60.0%"));
    }

    #[test]
    fn accounts_with_error_rendered() {
        let mut providers = BTreeMap::new();
        providers.insert(
            "opencodego".to_string(),
            ProviderResult {
                limits: None,
                accounts: vec![AccountResult {
                    email: "broken@example.com".to_string(),
                    plan: "".to_string(),
                    active: false,
                    limits: None,
                    error: Some("token expired".to_string()),
                }],
                error: None,
            },
        );

        let report = Report {
            checked_at: Utc::now(),
            providers,
        };

        let mut buf = vec![];
        render_bar(&mut buf, &report, &["opencodego"]).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("broken@example.com"));
        assert!(output.contains("token expired"));
    }
}
