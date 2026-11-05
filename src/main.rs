mod config;
mod osd;
mod polkit;
mod ryzen;

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::ryzen::RyzenAdj;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if let Err(err) = run() {
        eprintln!("\nError: {err:#}");
        std::process::exit(1);
    }
}

#[derive(Debug)]
enum Command {
    Apply {
        profile: Option<String>,
        no_osd: bool,
        dry_run: bool,
        timeout: Option<Duration>,
    },
    Next {
        no_osd: bool,
        dry_run: bool,
        timeout: Option<Duration>,
    },
    Preview {
        profile: String,
        timeout: Option<Duration>,
    },
    List,
    Status,
    Init,
    InstallPolkit,
    Help,
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (command, config_path) = parse_args(&args)?;

    match command {
        Command::Apply {
            profile,
            no_osd,
            dry_run,
            timeout,
        } => apply_profile(
            config_path.as_deref(),
            profile.as_deref(),
            no_osd,
            dry_run,
            timeout,
        ),
        Command::Next {
            no_osd,
            dry_run,
            timeout,
        } => apply_next_profile(config_path.as_deref(), no_osd, dry_run, timeout),
        Command::Preview { profile, timeout } => {
            preview_profile(config_path.as_deref(), &profile, timeout)
        }
        Command::List => list_profiles(config_path.as_deref()),
        Command::Status => show_status(),
        Command::Init => init_config(config_path),
        Command::InstallPolkit => install_polkit(),
        Command::Help => {
            print_help();
            Ok(())
        }
    }
}

fn parse_args(args: &[String]) -> Result<(Command, Option<PathBuf>)> {
    let mut config_path = None;
    let mut no_osd = false;
    let mut dry_run = false;
    let mut next = false;
    let mut timeout = None;
    let mut positional: Vec<String> = Vec::new();

    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-h" | "--help" => return Ok((Command::Help, config_path)),
            "-V" | "--version" => {
                println!("ryzen-power {VERSION}");
                std::process::exit(0);
            }
            "--no-osd" => no_osd = true,
            "--dry-run" => dry_run = true,
            "--next" => next = true,
            "--time" => {
                index += 1;
                let value = args
                    .get(index)
                    .context("--time requires a number of seconds")?;
                timeout = Some(parse_time_value(value)?);
            }
            _ if arg.starts_with("--time=") => {
                let value = &arg["--time=".len()..];
                timeout = Some(parse_time_value(value)?);
            }
            "--config" => {
                index += 1;
                let value = args.get(index).context("--config requires a file path")?;
                config_path = Some(PathBuf::from(value));
            }
            _ if arg.starts_with("--config=") => {
                let value = &arg["--config=".len()..];
                if value.is_empty() {
                    bail!("--config= requires a file path");
                }
                config_path = Some(PathBuf::from(value));
            }
            _ if arg.starts_with('-') => bail!("Unknown option: {arg}"),
            _ => positional.push(arg.clone()),
        }
        index += 1;
    }

    if next && !positional.is_empty() {
        bail!("--next cannot be combined with a profile or command");
    }

    if positional.is_empty() {
        let command = if next {
            Command::Next {
                no_osd,
                dry_run,
                timeout,
            }
        } else {
            Command::Apply {
                profile: None,
                no_osd,
                dry_run,
                timeout,
            }
        };
        return Ok((command, config_path));
    }

    let command = match positional[0].as_str() {
        "list" | "ls" | "profiles" => {
            ensure_no_extra(&positional, 1, "list")?;
            Command::List
        }
        "status" => {
            ensure_no_extra(&positional, 1, "status")?;
            Command::Status
        }
        "init" => {
            ensure_no_extra(&positional, 1, "init")?;
            Command::Init
        }
        "install-polkit" => {
            ensure_no_extra(&positional, 1, "install-polkit")?;
            Command::InstallPolkit
        }
        "preview" => {
            ensure_no_extra(&positional, 2, "preview")?;
            let profile = positional
                .get(1)
                .context("Usage: ryzen-power preview <profile>")?
                .clone();
            Command::Preview { profile, timeout }
        }
        "apply" => {
            ensure_no_extra(&positional, 2, "apply")?;
            Command::Apply {
                profile: positional.get(1).cloned(),
                no_osd,
                dry_run,
                timeout,
            }
        }
        _ => {
            ensure_no_extra(&positional, 1, "profile")?;
            Command::Apply {
                profile: Some(positional[0].clone()),
                no_osd,
                dry_run,
                timeout,
            }
        }
    };

    Ok((command, config_path))
}

fn ensure_no_extra(positional: &[String], allowed: usize, command: &str) -> Result<()> {
    if positional.len() > allowed {
        bail!(
            "Too many arguments for \"{}\": {}",
            command,
            positional[allowed..].join(" ")
        );
    }
    Ok(())
}

/// Parse a `--time` value: a positive number of seconds, which may be
/// fractional (`1.5` = 1500 ms).
fn parse_time_value(value: &str) -> Result<Duration> {
    let secs = value
        .parse::<f64>()
        .with_context(|| format!("Invalid --time value: {value}"))?;
    if !secs.is_finite() || secs <= 0.0 {
        bail!("--time must be greater than 0");
    }
    // Guard against values too large for Duration (panics above ~u64::MAX secs)
    // and against sub-millisecond values that would round to a zero hold.
    let millis = (secs * 1000.0).round();
    if millis < 1.0 {
        bail!("--time must be at least 0.001 seconds");
    }
    if millis > u64::MAX as f64 {
        bail!("--time value is too large: {value}");
    }
    Ok(Duration::from_millis(millis as u64))
}

fn apply_profile(
    config_path: Option<&std::path::Path>,
    profile_name: Option<&str>,
    no_osd: bool,
    dry_run: bool,
    timeout: Option<Duration>,
) -> Result<()> {
    let config = Config::load(config_path)?;
    let profile = match profile_name {
        Some(name) => config.resolve_profile(name).with_context(|| {
            format!(
                "Profile \"{name}\" was not found. Available profiles: {}",
                profile_names(&config)
            )
        })?,
        None => config.default_profile().with_context(
            || "No default profile is configured; specify one explicitly: ryzen-power <profile>",
        )?,
    };

    let ryzenadj = RyzenAdj::locate()?;

    if dry_run {
        println!("{}", ryzenadj.command_line(&profile.limits));
        return Ok(());
    }

    let outcome = match ryzenadj.apply(&profile.limits) {
        Ok(outcome) => outcome,
        Err(err) => {
            if config.general.osd && !no_osd && osd::display_available() {
                let _ = osd::show(&config, &osd::OsdData::error(&format!("{err:#}")), timeout);
            }
            return Err(err);
        }
    };

    if let Err(err) = config.record_applied_profile(&profile.name) {
        eprintln!("Warning: could not save the current profile: {err:#}");
    }

    println!(
        "Profile \"{}\" applied: {}",
        profile.label,
        profile.limits.summary()
    );

    if outcome.via_polkit && config.general.suggest_polkit && !polkit::installed() {
        println!();
        println!("Tip: install the polkit rule once to avoid the password prompt on every switch:");
        println!("  {}", polkit_install_command());
    }

    if config.general.osd
        && !no_osd
        && osd::display_available()
        && let Err(err) = osd::show(
            &config,
            &osd::OsdData::for_profile(&config, profile),
            timeout,
        )
    {
        eprintln!("Could not show the OSD: {err:#}");
    }

    Ok(())
}

/// Name of the profile that follows `current` in config-file order, wrapping
/// around at the end. An unknown or missing `current` starts the cycle at the
/// first profile.
fn next_profile_name(config: &Config, current: Option<&str>) -> Result<String> {
    let current = current.map(str::to_lowercase);
    let index = current.as_deref().and_then(|name| {
        config
            .profiles
            .iter()
            .position(|profile| profile.name == name)
    });
    let next = match index {
        Some(index) => (index + 1) % config.profiles.len(),
        None => 0,
    };
    config
        .profiles
        .get(next)
        .map(|profile| profile.name.clone())
        .context("The config contains no profiles")
}

/// Apply the profile after the last one applied by ryzen-power
/// (`ryzen-power --next`), recording the new one on success.
fn apply_next_profile(
    config_path: Option<&std::path::Path>,
    no_osd: bool,
    dry_run: bool,
    timeout: Option<Duration>,
) -> Result<()> {
    let config = Config::load(config_path)?;
    let current = config.last_applied_profile();
    let next = next_profile_name(&config, current.as_deref())?;
    apply_profile(config_path, Some(&next), no_osd, dry_run, timeout)
}

fn preview_profile(
    config_path: Option<&std::path::Path>,
    name: &str,
    timeout: Option<Duration>,
) -> Result<()> {
    let config = Config::load(config_path)?;
    let profile = config.resolve_profile(name).with_context(|| {
        format!(
            "Profile \"{name}\" was not found. Available profiles: {}",
            profile_names(&config)
        )
    })?;

    osd::show(
        &config,
        &osd::OsdData::for_profile(&config, profile),
        timeout,
    )
}

fn list_profiles(config_path: Option<&std::path::Path>) -> Result<()> {
    let config = Config::load(config_path)?;
    let default = config.general.default_profile.as_deref();

    println!("Config: {}", config.path.display());
    println!(
        "Theme:  {} (icons: {})",
        config.general.theme,
        config.themes_dir.display()
    );
    println!(
        "Last:   {}",
        config
            .last_applied_profile()
            .unwrap_or_else(|| "(not set)".to_string())
    );
    println!();
    for profile in &config.profiles {
        let marker = if Some(profile.name.as_str()) == default {
            "*"
        } else {
            " "
        };
        println!(
            "{} {}\t{}\t{}",
            marker,
            profile.name,
            profile.label,
            profile.limits.summary()
        );
    }
    println!();
    println!("Apply:       ryzen-power <profile>");
    println!("Next:        ryzen-power --next");
    println!("Preview OSD: ryzen-power preview <profile>");
    Ok(())
}

fn show_status() -> Result<()> {
    let ryzenadj = RyzenAdj::locate()?;
    let output = ryzenadj.status()?;
    print!("{output}");
    if !output.ends_with('\n') {
        println!();
    }
    Ok(())
}

fn init_config(path: Option<PathBuf>) -> Result<()> {
    let config = Config::load(path.as_deref())?;
    println!("Config: {}", config.path.display());
    println!("Themes: {}", config.themes_dir.display());
    println!();
    println!("Edit the config to change profiles, layout, and the active theme.");
    println!(
        "Custom icons: {}/<theme>/<profile>.img",
        config.themes_dir.display()
    );
    Ok(())
}

fn install_polkit() -> Result<()> {
    let ryzenadj = RyzenAdj::locate()?;
    polkit::install(ryzenadj.path())
}

fn profile_names(config: &Config) -> String {
    config
        .profiles
        .iter()
        .map(|profile| profile.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn polkit_install_command() -> String {
    match std::env::current_exe() {
        Ok(path) => format!("sudo {} install-polkit", path.display()),
        Err(_) => "sudo ryzen-power install-polkit".to_string(),
    }
}

fn print_help() {
    println!(
        r#"ryzen-power {VERSION} — AMD Ryzen power-limit profile switcher

Usage:
  ryzen-power [profile] [--no-osd] [--dry-run] [--time SECONDS] [--config FILE]
  ryzen-power --next [--no-osd] [--dry-run] [--time SECONDS]
  ryzen-power preview <profile> [--time SECONDS]
  ryzen-power list
  ryzen-power status
  ryzen-power init
  ryzen-power install-polkit

Commands:
  <profile>          Apply a profile and show the OSD
  --next             Apply the next profile in config order (cycles)
  preview <profile>  Show the OSD without changing any limits
  list               List profiles from the config
  status             Show current values from ryzenadj -i
  init               Create the config and themes directory
  install-polkit     Install the polkit rule (requires root)

Options:
  --next             Cycle to the next profile (eco -> balanced -> performance -> ...)
  --no-osd           Do not show the OSD
  --dry-run          Print the ryzenadj command only
  --time SECONDS     Override the OSD display time for this run (may be fractional)
  --config FILE      Use another configuration file
  -h, --help         Show this help
  -V, --version      Show the version

Examples:
  ryzen-power eco
  ryzen-power balanced
  ryzen-power performance
  ryzen-power --next
  ryzen-power --next --time 2
  ryzen-power preview balanced
  ryzen-power preview balanced --time 3
  ryzen-power preview balanced --time 1.5

The config is stored in ~/.config/ryzen-power/config.conf.
If no profile is given, the default from the [general] section is used.
--next follows the profile order in the config and remembers the last applied
profile in ~/.config/ryzen-power/config.state."#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn parses_time_for_apply() {
        let (command, _) = parse_args(&args(&["--time", "3", "balanced"])).unwrap();
        match command {
            Command::Apply {
                profile, timeout, ..
            } => {
                assert_eq!(profile.as_deref(), Some("balanced"));
                assert_eq!(timeout, Some(Duration::from_secs(3)));
            }
            other => panic!("expected apply command, got {other:?}"),
        }
    }

    #[test]
    fn parses_fractional_time() {
        let (command, _) = parse_args(&args(&["--time", "1.5", "balanced"])).unwrap();
        match command {
            Command::Apply { timeout, .. } => {
                assert_eq!(timeout, Some(Duration::from_millis(1500)));
            }
            other => panic!("expected apply command, got {other:?}"),
        }

        let (command, _) = parse_args(&args(&["preview", "eco", "--time=0.25"])).unwrap();
        match command {
            Command::Preview { timeout, .. } => {
                assert_eq!(timeout, Some(Duration::from_millis(250)));
            }
            other => panic!("expected preview command, got {other:?}"),
        }
    }

    #[test]
    fn parses_time_for_preview() {
        let (command, _) = parse_args(&args(&["preview", "eco", "--time=2"])).unwrap();
        match command {
            Command::Preview { profile, timeout } => {
                assert_eq!(profile, "eco");
                assert_eq!(timeout, Some(Duration::from_secs(2)));
            }
            other => panic!("expected preview command, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_time_values() {
        for value in ["0", "abc", "-1", "-0.5", "nan", "inf", "0.0000001"] {
            let error = parse_args(&args(&["--time", value, "eco"])).unwrap_err();
            assert!(
                format!("{error:#}").contains("time"),
                "unexpected error for {value:?}: {error:#}"
            );
        }
    }

    #[test]
    fn requires_time_value() {
        let error = parse_args(&args(&["--time"])).unwrap_err();
        assert!(format!("{error:#}").contains("--time requires"));
    }

    #[test]
    fn parses_next_option() {
        let (command, _) = parse_args(&args(&["--next", "--time", "2"])).unwrap();
        match command {
            Command::Next {
                no_osd,
                dry_run,
                timeout,
            } => {
                assert!(!no_osd);
                assert!(!dry_run);
                assert_eq!(timeout, Some(Duration::from_secs(2)));
            }
            other => panic!("expected next command, got {other:?}"),
        }
    }

    #[test]
    fn rejects_next_with_profile() {
        let error = parse_args(&args(&["--next", "eco"])).unwrap_err();
        assert!(format!("{error:#}").contains("--next"));
    }

    #[test]
    fn next_profile_cycles_in_config_order() {
        let config =
            Config::parse(PathBuf::from("test.conf"), crate::config::DEFAULT_CONFIG).unwrap();

        // Unknown state starts the cycle at the first profile.
        assert_eq!(next_profile_name(&config, None).unwrap(), "eco");
        assert_eq!(next_profile_name(&config, Some("ghost")).unwrap(), "eco");
        // Then it advances in config-file order and wraps around.
        assert_eq!(next_profile_name(&config, Some("eco")).unwrap(), "balanced");
        assert_eq!(
            next_profile_name(&config, Some("balanced")).unwrap(),
            "performance"
        );
        assert_eq!(
            next_profile_name(&config, Some("performance")).unwrap(),
            "eco"
        );
    }
}
