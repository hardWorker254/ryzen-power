use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::ryzen::is_root;

pub const ACTION_ID: &str = "org.ryzenpower.apply";
pub const ACTION_PATH: &str = "/usr/share/polkit-1/actions/org.ryzenpower.apply.policy";
pub const RULE_PATH: &str = "/etc/polkit-1/rules.d/49-ryzen-power.rules";

pub fn installed() -> bool {
    Path::new(ACTION_PATH).exists() && Path::new(RULE_PATH).exists()
}

pub fn install(ryzenadj: &Path) -> Result<()> {
    if !is_root() {
        let command = std::env::current_exe()
            .map(|path| format!("sudo {} install-polkit", path.display()))
            .unwrap_or_else(|_| "sudo ryzen-power install-polkit".to_string());
        bail!(
            "Root privileges are required to install the polkit rule.\n\
             Run:\n  {command}"
        );
    }

    let ryzenadj = fs::canonicalize(ryzenadj).unwrap_or_else(|_| ryzenadj.to_path_buf());
    let path = ryzenadj.to_string_lossy();
    if !path.starts_with('/') {
        bail!("Could not determine the absolute path to ryzenadj");
    }

    let action = action_policy(&path);
    let rule = rule_content();

    write_root_file(Path::new(ACTION_PATH), &action)?;
    write_root_file(Path::new(RULE_PATH), &rule)?;

    println!("Installed:");
    println!("  {ACTION_PATH}");
    println!("  {RULE_PATH}");
    println!();
    println!(
        "Members of the wheel/sudo groups can now change Ryzen limits through pkexec without a password."
    );
    println!("Check: pkexec {} -i", ryzenadj.display());
    Ok(())
}

fn action_policy(ryzenadj: &str) -> String {
    let path = xml_escape(ryzenadj);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE policyconfig PUBLIC
 "-//freedesktop//DTD PolicyKit Policy Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/PolicyKit/1/policyconfig.dtd">
<policyconfig>
  <vendor>ryzen-power</vendor>
  <action id="{ACTION_ID}">
    <description>Change AMD Ryzen power limits</description>
    <message>Authentication is required to change AMD Ryzen power limits</message>
    <defaults>
      <allow_any>no</allow_any>
      <allow_inactive>no</allow_inactive>
      <allow_active>auth_admin_keep</allow_active>
    </defaults>
    <annotate key="org.freedesktop.policykit.exec.path">{path}</annotate>
  </action>
</policyconfig>
"#
    )
}

fn rule_content() -> String {
    format!(
        r#"/* Installed by ryzen-power.
 * Allows administrators to change Ryzen limits without a password prompt.
 * Remove with: sudo rm {RULE_PATH}
 */
polkit.addRule(function(action, subject) {{
    if (action.id !== "{ACTION_ID}") {{
        return polkit.Result.NOT_HANDLED;
    }}
    if (!subject.local || !subject.active) {{
        return polkit.Result.NOT_HANDLED;
    }}
    if (subject.isInGroup("wheel") || subject.isInGroup("sudo")) {{
        return polkit.Result.YES;
    }}
    return polkit.Result.NOT_HANDLED;
}});
"#
    )
}

fn write_root_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create directory {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("Could not write {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o644))
        .with_context(|| format!("Could not set permissions on {}", path.display()))?;
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_points_to_ryzenadj() {
        let policy = action_policy("/usr/bin/ryzenadj");
        assert!(policy.contains(ACTION_ID));
        assert!(policy.contains(
            r#"<annotate key="org.freedesktop.policykit.exec.path">/usr/bin/ryzenadj</annotate>"#
        ));
        assert!(policy.contains("<allow_active>auth_admin_keep</allow_active>"));
    }

    #[test]
    fn rule_mentions_action_and_admin_groups() {
        let rule = rule_content();
        assert!(rule.contains(ACTION_ID));
        assert!(rule.contains("wheel"));
        assert!(rule.contains("polkit.Result.YES"));
    }
}
