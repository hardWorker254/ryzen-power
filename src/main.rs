mod config;
mod osd;
mod polkit;
mod ryzen;

use std::env;
use std::path::PathBuf;

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
    },
    Preview {
        profile: String,
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
        } => apply_profile(config_path.as_deref(), profile.as_deref(), no_osd, dry_run),
        Command::Preview { profile } => preview_profile(config_path.as_deref(), &profile),
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

    if positional.is_empty() {
        return Ok((
            Command::Apply {
                profile: None,
                no_osd,
                dry_run,
            },
            config_path,
        ));
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
            Command::Preview { profile }
        }
        "apply" => {
            ensure_no_extra(&positional, 2, "apply")?;
            Command::Apply {
                profile: positional.get(1).cloned(),
                no_osd,
                dry_run,
            }
        }
        _ => {
            ensure_no_extra(&positional, 1, "profile")?;
            Command::Apply {
                profile: Some(positional[0].clone()),
                no_osd,
                dry_run,
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

fn apply_profile(
    config_path: Option<&std::path::Path>,
    profile_name: Option<&str>,
    no_osd: bool,
    dry_run: bool,
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
                let _ = osd::show(&config, &osd::OsdData::error(&format!("{err:#}")));
            }
            return Err(err);
        }
    };

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
        && let Err(err) = osd::show(&config, &osd::OsdData::for_profile(&config, profile))
    {
        eprintln!("Could not show the OSD: {err:#}");
    }

    Ok(())
}

fn preview_profile(config_path: Option<&std::path::Path>, name: &str) -> Result<()> {
    let config = Config::load(config_path)?;
    let profile = config.resolve_profile(name).with_context(|| {
        format!(
            "Profile \"{name}\" was not found. Available profiles: {}",
            profile_names(&config)
        )
    })?;

    osd::show(&config, &osd::OsdData::for_profile(&config, profile))
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
  ryzen-power [profile] [--no-osd] [--dry-run] [--config FILE]
  ryzen-power preview <profile>
  ryzen-power list
  ryzen-power status
  ryzen-power init
  ryzen-power install-polkit

Commands:
  <profile>          Apply a profile and show the OSD
  preview <profile>  Show the OSD without changing any limits
  list               List profiles from the config
  status             Show current values from ryzenadj -i
  init               Create the config and themes directory
  install-polkit     Install the polkit rule (requires root)

Options:
  --no-osd           Do not show the OSD
  --dry-run          Print the ryzenadj command only
  --config FILE      Use another configuration file
  -h, --help         Show this help
  -V, --version      Show the version

Examples:
  ryzen-power eco
  ryzen-power balanced
  ryzen-power performance
  ryzen-power preview balanced

The config is stored in ~/.config/ryzen-power/config.conf.
If no profile is given, the default from the [general] section is used."#
    );
}
