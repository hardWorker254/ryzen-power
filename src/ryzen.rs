use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use anyhow::{Context, Result, bail};

use crate::config::Limits;

#[derive(Debug, Clone)]
pub struct RyzenAdj {
    path: PathBuf,
}

#[derive(Debug, Clone, Copy)]
pub struct ApplyOutcome {
    pub via_polkit: bool,
}

impl RyzenAdj {
    pub fn locate() -> Result<Self> {
        let mut candidates = vec![
            PathBuf::from("/usr/bin/ryzenadj"),
            PathBuf::from("/usr/sbin/ryzenadj"),
            PathBuf::from("/usr/local/bin/ryzenadj"),
        ];

        if let Some(path) = env::var_os("PATH") {
            for dir in env::split_paths(&path) {
                candidates.push(dir.join("ryzenadj"));
            }
        }

        for candidate in candidates {
            if is_executable(&candidate) {
                let path = fs::canonicalize(&candidate).unwrap_or(candidate);
                return Ok(Self { path });
            }
        }

        bail!(
            "ryzenadj was not found.\n\
             Install the ryzenadj package (Artix/Arch: sudo pacman -S ryzenadj)."
        )
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn apply(&self, limits: &Limits) -> Result<ApplyOutcome> {
        let args = limits.ryzenadj_args();
        if args.is_empty() {
            bail!("The profile has no limits to apply");
        }

        let direct = self.run_direct(&args)?;
        if direct.status.success() {
            return Ok(ApplyOutcome { via_polkit: false });
        }

        if is_permission_error(&direct) && !is_root() {
            let pkexec = self.run_pkexec(&args)?;
            if pkexec.status.success() {
                return Ok(ApplyOutcome { via_polkit: true });
            }
            bail!(format_pkexec_error(&pkexec));
        }

        bail!(format_ryzen_error(&direct));
    }

    pub fn status(&self) -> Result<String> {
        let args = vec!["-i".to_string()];
        let direct = self.run_direct(&args)?;
        if direct.status.success() {
            return Ok(combine_output(&direct));
        }

        if is_permission_error(&direct) && !is_root() {
            let pkexec = self.run_pkexec(&args)?;
            if pkexec.status.success() {
                return Ok(combine_output(&pkexec));
            }
            bail!(format_pkexec_error(&pkexec));
        }

        // Some ryzenadj builds print useful info but return a non-zero code.
        let text = combine_output(&direct);
        if text.trim().is_empty() {
            bail!(format_ryzen_error(&direct));
        }
        Ok(text)
    }

    pub fn command_line(&self, limits: &Limits) -> String {
        let mut parts = Vec::new();
        if !is_root() {
            parts.push("pkexec".to_string());
        }
        parts.push(shell_quote(&self.path.to_string_lossy()));
        parts.extend(limits.ryzenadj_args());
        parts.join(" ")
    }

    fn run_direct(&self, args: &[String]) -> Result<Output> {
        Command::new(&self.path)
            .args(args)
            .output()
            .with_context(|| format!("Could not run {}", self.path.display()))
    }

    fn run_pkexec(&self, args: &[String]) -> Result<Output> {
        let pkexec = find_in_path("pkexec")
            .context("pkexec was not found. Install polkit or run the program through sudo.")?;

        Command::new(pkexec)
            .arg(&self.path)
            .args(args)
            .output()
            .context("Could not run pkexec")
    }
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn is_permission_error(output: &Output) -> bool {
    let text = combine_output(output).to_ascii_lowercase();
    text.contains("permission")
        || text.contains("os_access")
        || text.contains("unable to init ryzenadj")
        || text.contains("access denied")
}

pub fn combine_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    match (stdout.trim(), stderr.trim()) {
        ("", "") => String::new(),
        ("", stderr) => stderr.to_string(),
        (stdout, "") => stdout.to_string(),
        (stdout, stderr) => format!("{stdout}\n{stderr}"),
    }
}

fn format_ryzen_error(output: &Output) -> String {
    let text = combine_output(output);
    if text.trim().is_empty() {
        format!("ryzenadj exited with code {:?}", output.status.code())
    } else {
        format!("ryzenadj failed:\n{}", trim_lines(&text, 10))
    }
}

fn format_pkexec_error(output: &Output) -> String {
    let text = combine_output(output);
    let lower = text.to_ascii_lowercase();

    if lower.contains("authentication agent") {
        return format!(
            "Could not get root privileges: no polkit authentication agent was found.\n\
             Install and start an agent (for example hyprpolkitagent or polkit-gnome), \
             or install the rule once:\n  {}",
            install_command()
        );
    }

    if lower.contains("dismissed")
        || lower.contains("not authorized")
        || output.status.code() == Some(126)
    {
        return "Authentication was cancelled or privileges are insufficient.".to_string();
    }

    if lower.contains("not found") {
        return format!("pkexec could not run ryzenadj:\n{}", trim_lines(&text, 8));
    }

    if text.trim().is_empty() {
        format!("pkexec exited with code {:?}", output.status.code())
    } else {
        format!("Could not apply the settings:\n{}", trim_lines(&text, 10))
    }
}

fn trim_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let mut result = lines
        .iter()
        .take(max_lines)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    if lines.len() > max_lines {
        result.push_str("\n…");
    }
    result
}

fn install_command() -> String {
    match env::current_exe() {
        Ok(path) => format!("sudo {} install-polkit", path.display()),
        Err(_) => "sudo ryzen-power install-polkit".to_string(),
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._-".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
