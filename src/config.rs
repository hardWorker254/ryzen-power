use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

pub const DEFAULT_CONFIG: &str = r#"# ryzen-power configuration
#
# Power values are in milliwatts (mW). Watts are also accepted: 45w.
# Temperature is in degrees Celsius.
# Values are passed to ryzenadj unchanged; ryzenadj and the firmware
# decide what is acceptable.
# Current limits (mA or A): tdc/vrm-current, edc/vrmmax-current,
#   tdc_soc/vrmsoc-current, edc_soc/vrmsocmax-current, vrmgfx-current,
#   vrmcvip-current, vrmgfxmax_current, psi0-current, psi3cpu_current,
#   psi0soc-current, psi3gfx_current.
# Clocks (MHz): max/min-socclk-frequency, max/min-fclk-frequency,
#   max/min-vcn, max/min-lclk, max/min-gfxclk, gfx-clk, oc-clk.
# OC/curve: oc-volt, enable-oc, disable-oc (true/false),
#   set-coall, set-coper, set-cogfx. Modes: power-saving, max-performance.
# Other: prochot-deassertion-ramp, slow_time, stapm_time (s).
# All ryzenadj options are supported; keys match ryzenadj long option names
# (hyphens or underscores). Values pass through to ryzenadj unchanged.
#
# Profiles are selected by name:
#   ryzen-power eco
#   ryzen-power balanced
#   ryzen-power performance
#
# List profiles:        ryzen-power list
# Preview the OSD:      ryzen-power preview balanced
# Next profile:         ryzen-power --next
#
# Custom OSD icons live in:
#   ~/.config/ryzen-power/themes/<theme>/<profile>.img
# Select a theme with the "theme" option below.

[general]
# Profile used when ryzen-power is started without an argument.
default = balanced

# Custom icon theme directory name. Icons are loaded from
# ~/.config/ryzen-power/themes/<theme>/<profile>.img
# Missing icons fall back to themes/default/ and then to built-in glyphs.
theme = default

# Show the translucent OSD when a profile is applied or previewed.
osd = true

# Screen edge: bottom | center | top
osd_position = bottom
# Distance from the top/bottom edge in pixels (ignored for center).
osd_margin = 110
# Minimum OSD width in pixels.
osd_width = 380
# How long the OSD stays fully visible, in milliseconds.
# The --time CLI option overrides this for one run.
osd_timeout_ms = 1400

# Card appearance.
background_color = #101218
background_opacity = 0.88
border_color = #ffffff
border_opacity = 0.10
border_width = 1
border_radius = 18
# Horizontal/vertical padding inside the card, in pixels.
padding_x = 18
padding_y = 13
# Leave empty to use the system default font.
font_family =

# Text colors.
text_color = #f5f7fb
subtitle_color =
subtitle_opacity = 0.62
badge_color =

# Icon placement: left | right | top | none
icon_position = left
icon_size = 24
# Icon background: circle | rounded | none
icon_background = circle
icon_padding = 9

# Text and bar visibility.
show_badge = true
show_subtitle = true
show_bar = true
bar_height = 8
# Bar position: bottom | top
bar_position = bottom
# Text alignment: left | center
text_align = left

# Font sizes in pixels.
title_size = 16
subtitle_size = 12
badge_size = 18

# Suggest the one-time polkit rule installation after a password prompt.
suggest_polkit = true

[eco]
label = Power Saver
icon = power-profile-power-saver-symbolic
color = #4ccb74
stapm = 25000
slow = 30000
fast = 35000
tctl = 85

[balanced]
label = Balanced
icon = power-profile-balanced-symbolic
color = #58a6ff
stapm = 45000
slow = 52000
fast = 62000
tctl = 90

[performance]
label = Performance
icon = power-profile-performance-symbolic
color = #ff9d4d
stapm = 54000
slow = 62000
fast = 72000
tctl = 95
"#;

pub const THEMES_README: &str = r#"ryzen-power custom icons
========================

Create theme directories here and put one image per profile, for example:

  themes/my-theme/eco.img
  themes/my-theme/balanced.img
  themes/my-theme/performance.img

File names must match the profile section names from config.conf.
PNG, JPEG, WebP and SVG are supported. The .img extension is recommended,
but .png, .jpg, .jpeg, .webp and .svg also work.

Select the theme in ~/.config/ryzen-power/config.conf:

  [general]
  theme = my-theme

If an icon is missing, ryzen-power falls back to themes/default/<profile>.*
and then to its built-in glyphs.
"#;

#[derive(Debug, Clone)]
pub struct Config {
    pub path: PathBuf,
    pub themes_dir: PathBuf,
    pub general: General,
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OsdPosition {
    #[default]
    Bottom,
    Center,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconPosition {
    #[default]
    Left,
    Right,
    Top,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconBackground {
    #[default]
    Circle,
    Rounded,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarPosition {
    #[default]
    Bottom,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
}

#[derive(Debug, Clone)]
pub struct General {
    pub default_profile: Option<String>,
    pub theme: String,
    pub osd: bool,
    pub osd_timeout_ms: u64,
    pub osd_position: OsdPosition,
    pub osd_margin: i32,
    pub osd_width: i32,
    pub background_color: Rgb,
    pub background_opacity: f64,
    pub border_color: Rgb,
    pub border_opacity: f64,
    pub border_width: i32,
    pub border_radius: i32,
    pub padding_x: i32,
    pub padding_y: i32,
    pub font_family: String,
    pub text_color: Rgb,
    pub subtitle_color: Option<Rgb>,
    pub subtitle_opacity: f64,
    pub badge_color: Option<Rgb>,
    pub icon_position: IconPosition,
    pub icon_size: i32,
    pub icon_background: IconBackground,
    pub icon_padding: i32,
    pub show_badge: bool,
    pub show_subtitle: bool,
    pub show_bar: bool,
    pub bar_height: i32,
    pub bar_position: BarPosition,
    pub text_align: TextAlign,
    pub title_size: i32,
    pub subtitle_size: i32,
    pub badge_size: i32,
    pub suggest_polkit: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            default_profile: None,
            theme: "default".to_string(),
            osd: true,
            osd_timeout_ms: 1400,
            osd_position: OsdPosition::Bottom,
            osd_margin: 110,
            osd_width: 380,
            background_color: Rgb(0x10, 0x12, 0x18),
            background_opacity: 0.88,
            border_color: Rgb(0xff, 0xff, 0xff),
            border_opacity: 0.10,
            border_width: 1,
            border_radius: 18,
            padding_x: 18,
            padding_y: 13,
            font_family: String::new(),
            text_color: Rgb(0xf5, 0xf7, 0xfb),
            subtitle_color: None,
            subtitle_opacity: 0.62,
            badge_color: None,
            icon_position: IconPosition::Left,
            icon_size: 24,
            icon_background: IconBackground::Circle,
            icon_padding: 9,
            show_badge: true,
            show_subtitle: true,
            show_bar: true,
            bar_height: 8,
            bar_position: BarPosition::Bottom,
            text_align: TextAlign::Left,
            title_size: 16,
            subtitle_size: 12,
            badge_size: 18,
            suggest_polkit: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub label: String,
    pub icon: String,
    pub color: Rgb,
    pub limits: Limits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    pub fn as_f64(self) -> (f64, f64, f64) {
        (
            self.0 as f64 / 255.0,
            self.1 as f64 / 255.0,
            self.2 as f64 / 255.0,
        )
    }

    fn parse(value: &str) -> Option<Self> {
        let value = value.trim().trim_start_matches('#');
        match value.len() {
            3 => {
                let mut chars = value.chars();
                let r = chars.next()?.to_digit(16)? as u8;
                let g = chars.next()?.to_digit(16)? as u8;
                let b = chars.next()?.to_digit(16)? as u8;
                Some(Self(r * 17, g * 17, b * 17))
            }
            6 => {
                let r = u8::from_str_radix(&value[0..2], 16).ok()?;
                let g = u8::from_str_radix(&value[2..4], 16).ok()?;
                let b = u8::from_str_radix(&value[4..6], 16).ok()?;
                Some(Self(r, g, b))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Limits {
    /// Sustained Power Limit, mW.
    pub stapm: Option<u32>,
    /// Average PPT limit, mW.
    pub slow: Option<u32>,
    /// Actual/boost PPT limit, mW.
    pub fast: Option<u32>,
    /// APU PPT Slow Power limit for A+A dGPU platforms, mW.
    pub apu_slow_limit: Option<u32>,
    /// Skin Temperature Power Limit, mW.
    pub skin_temp_limit: Option<u32>,
    /// Tctl temperature limit, °C.
    pub tctl: Option<u32>,
    /// STT LIMIT APU skin temperature, °C.
    pub apu_skin_temp: Option<u32>,
    /// STT LIMIT dGPU skin temperature, °C.
    pub dgpu_skin_temp: Option<u32>,
    /// Slow PPT constant time, s.
    pub slow_time: Option<u32>,
    /// STAPM constant time, s.
    pub stapm_time: Option<u32>,
    /// VRM current limit VDD, mA.
    pub tdc: Option<u32>,
    /// VRM SoC current limit, mA.
    pub tdc_soc: Option<u32>,
    /// VRM GFX current limit, mA.
    pub vrmgfx_current: Option<u32>,
    /// VRM CVIP current limit, mA.
    pub vrmcvip_current: Option<u32>,
    /// VRM maximum current limit VDD, mA.
    pub edc: Option<u32>,
    /// VRM SoC maximum current limit, mA.
    pub edc_soc: Option<u32>,
    /// VRM GFX maximum current limit, mA.
    pub vrmgfxmax_current: Option<u32>,
    /// PSI0 VDD current limit, mA.
    pub psi0_current: Option<u32>,
    /// PSI3 CPU current limit, mA.
    pub psi3cpu_current: Option<u32>,
    /// PSI0 SoC current limit, mA.
    pub psi0soc_current: Option<u32>,
    /// PSI3 GFX current limit, mA.
    pub psi3gfx_current: Option<u32>,
    /// Maximum SoC clock frequency, MHz.
    pub max_socclk_frequency: Option<u32>,
    /// Minimum SoC clock frequency, MHz.
    pub min_socclk_frequency: Option<u32>,
    /// Maximum transmission (CPU-GPU) frequency, MHz.
    pub max_fclk_frequency: Option<u32>,
    /// Minimum transmission (CPU-GPU) frequency, MHz.
    pub min_fclk_frequency: Option<u32>,
    /// Maximum VCN (video core next), MHz.
    pub max_vcn: Option<u32>,
    /// Minimum VCN (video core next), MHz.
    pub min_vcn: Option<u32>,
    /// Maximum data launch clock, MHz.
    pub max_lclk: Option<u32>,
    /// Minimum data launch clock, MHz.
    pub min_lclk: Option<u32>,
    /// Maximum GFX clock, MHz.
    pub max_gfxclk: Option<u32>,
    /// Minimum GFX clock, MHz.
    pub min_gfxclk: Option<u32>,
    /// Ramp time after Prochot is deasserted.
    pub prochot_deassertion_ramp: Option<u32>,
    /// Forced GFX clock speed (Renoir only), MHz.
    pub gfx_clk: Option<u32>,
    /// Forced core clock speed (Renoir and up only), MHz.
    pub oc_clk: Option<u32>,
    /// Forced core VID (Renoir and up only).
    pub oc_volt: Option<u32>,
    /// Enable overclocking (Renoir and up only).
    pub enable_oc: Option<bool>,
    /// Disable overclocking (Renoir and up only).
    pub disable_oc: Option<bool>,
    /// All-core curve optimiser.
    pub set_coall: Option<u32>,
    /// Per-core curve optimiser.
    pub set_coper: Option<u32>,
    /// iGPU curve optimiser.
    pub set_cogfx: Option<u32>,
    /// Hidden power-saving mode.
    pub power_saving: Option<bool>,
    /// Hidden max-performance mode.
    pub max_performance: Option<bool>,
}

impl Limits {
    pub fn is_empty(&self) -> bool {
        self.stapm.is_none()
            && self.slow.is_none()
            && self.fast.is_none()
            && self.apu_slow_limit.is_none()
            && self.skin_temp_limit.is_none()
            && self.tctl.is_none()
            && self.apu_skin_temp.is_none()
            && self.dgpu_skin_temp.is_none()
            && self.slow_time.is_none()
            && self.stapm_time.is_none()
            && self.tdc.is_none()
            && self.tdc_soc.is_none()
            && self.vrmgfx_current.is_none()
            && self.vrmcvip_current.is_none()
            && self.edc.is_none()
            && self.edc_soc.is_none()
            && self.vrmgfxmax_current.is_none()
            && self.psi0_current.is_none()
            && self.psi3cpu_current.is_none()
            && self.psi0soc_current.is_none()
            && self.psi3gfx_current.is_none()
            && self.max_socclk_frequency.is_none()
            && self.min_socclk_frequency.is_none()
            && self.max_fclk_frequency.is_none()
            && self.min_fclk_frequency.is_none()
            && self.max_vcn.is_none()
            && self.min_vcn.is_none()
            && self.max_lclk.is_none()
            && self.min_lclk.is_none()
            && self.max_gfxclk.is_none()
            && self.min_gfxclk.is_none()
            && self.prochot_deassertion_ramp.is_none()
            && self.gfx_clk.is_none()
            && self.oc_clk.is_none()
            && self.oc_volt.is_none()
            && self.enable_oc != Some(true)
            && self.disable_oc != Some(true)
            && self.set_coall.is_none()
            && self.set_coper.is_none()
            && self.set_cogfx.is_none()
            && self.power_saving != Some(true)
            && self.max_performance != Some(true)
    }

    pub fn ryzenadj_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(v) = self.stapm {
            args.push(format!("--stapm-limit={v}"));
        }
        if let Some(v) = self.fast {
            args.push(format!("--fast-limit={v}"));
        }
        if let Some(v) = self.slow {
            args.push(format!("--slow-limit={v}"));
        }
        if let Some(v) = self.slow_time {
            args.push(format!("--slow-time={v}"));
        }
        if let Some(v) = self.stapm_time {
            args.push(format!("--stapm-time={v}"));
        }
        if let Some(v) = self.tctl {
            args.push(format!("--tctl-temp={v}"));
        }
        if let Some(v) = self.tdc {
            args.push(format!("--vrm-current={v}"));
        }
        if let Some(v) = self.tdc_soc {
            args.push(format!("--vrmsoc-current={v}"));
        }
        if let Some(v) = self.vrmgfx_current {
            args.push(format!("--vrmgfx-current={v}"));
        }
        if let Some(v) = self.vrmcvip_current {
            args.push(format!("--vrmcvip-current={v}"));
        }
        if let Some(v) = self.edc {
            args.push(format!("--vrmmax-current={v}"));
        }
        if let Some(v) = self.edc_soc {
            args.push(format!("--vrmsocmax-current={v}"));
        }
        if let Some(v) = self.vrmgfxmax_current {
            args.push(format!("--vrmgfxmax_current={v}"));
        }
        if let Some(v) = self.psi0_current {
            args.push(format!("--psi0-current={v}"));
        }
        if let Some(v) = self.psi3cpu_current {
            args.push(format!("--psi3cpu_current={v}"));
        }
        if let Some(v) = self.psi0soc_current {
            args.push(format!("--psi0soc-current={v}"));
        }
        if let Some(v) = self.psi3gfx_current {
            args.push(format!("--psi3gfx_current={v}"));
        }
        if let Some(v) = self.max_socclk_frequency {
            args.push(format!("--max-socclk-frequency={v}"));
        }
        if let Some(v) = self.min_socclk_frequency {
            args.push(format!("--min-socclk-frequency={v}"));
        }
        if let Some(v) = self.max_fclk_frequency {
            args.push(format!("--max-fclk-frequency={v}"));
        }
        if let Some(v) = self.min_fclk_frequency {
            args.push(format!("--min-fclk-frequency={v}"));
        }
        if let Some(v) = self.max_vcn {
            args.push(format!("--max-vcn={v}"));
        }
        if let Some(v) = self.min_vcn {
            args.push(format!("--min-vcn={v}"));
        }
        if let Some(v) = self.max_lclk {
            args.push(format!("--max-lclk={v}"));
        }
        if let Some(v) = self.min_lclk {
            args.push(format!("--min-lclk={v}"));
        }
        if let Some(v) = self.max_gfxclk {
            args.push(format!("--max-gfxclk={v}"));
        }
        if let Some(v) = self.min_gfxclk {
            args.push(format!("--min-gfxclk={v}"));
        }
        if let Some(v) = self.prochot_deassertion_ramp {
            args.push(format!("--prochot-deassertion-ramp={v}"));
        }
        if let Some(v) = self.apu_skin_temp {
            args.push(format!("--apu-skin-temp={v}"));
        }
        if let Some(v) = self.dgpu_skin_temp {
            args.push(format!("--dgpu-skin-temp={v}"));
        }
        if let Some(v) = self.apu_slow_limit {
            args.push(format!("--apu-slow-limit={v}"));
        }
        if let Some(v) = self.skin_temp_limit {
            args.push(format!("--skin-temp-limit={v}"));
        }
        if let Some(v) = self.gfx_clk {
            args.push(format!("--gfx-clk={v}"));
        }
        if let Some(v) = self.oc_clk {
            args.push(format!("--oc-clk={v}"));
        }
        if let Some(v) = self.oc_volt {
            args.push(format!("--oc-volt={v}"));
        }
        if self.enable_oc == Some(true) {
            args.push("--enable-oc".to_string());
        }
        if self.disable_oc == Some(true) {
            args.push("--disable-oc".to_string());
        }
        if let Some(v) = self.set_coall {
            args.push(format!("--set-coall={v}"));
        }
        if let Some(v) = self.set_coper {
            args.push(format!("--set-coper={v}"));
        }
        if let Some(v) = self.set_cogfx {
            args.push(format!("--set-cogfx={v}"));
        }
        if self.power_saving == Some(true) {
            args.push("--power-saving".to_string());
        }
        if self.max_performance == Some(true) {
            args.push("--max-performance".to_string());
        }
        args
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        if let Some(v) = self.stapm {
            parts.push(format!("STAPM {}", watts(v)));
        }
        match (self.slow, self.fast) {
            (Some(slow), Some(fast)) => {
                parts.push(format!("PPT {}/{} W", watts_value(slow), watts_value(fast)));
            }
            (Some(slow), None) => parts.push(format!("PPT slow {}", watts(slow))),
            (None, Some(fast)) => parts.push(format!("PPT fast {}", watts(fast))),
            (None, None) => {}
        }
        if let Some(v) = self.apu_slow_limit {
            parts.push(format!("APU slow {}", watts(v)));
        }
        if let Some(v) = self.skin_temp_limit {
            parts.push(format!("Skin power {}", watts(v)));
        }
        if let Some(v) = self.tctl {
            parts.push(format!("{v} °C"));
        }
        if let Some(v) = self.apu_skin_temp {
            parts.push(format!("APU skin {v} °C"));
        }
        if let Some(v) = self.dgpu_skin_temp {
            parts.push(format!("dGPU skin {v} °C"));
        }

        if parts.is_empty() {
            "no limits".to_string()
        } else {
            parts.join(" · ")
        }
    }

    pub fn fast_watts(&self) -> Option<String> {
        self.fast.map(watts)
    }
}

pub fn watts(mw: u32) -> String {
    format!("{} W", watts_value(mw))
}

fn watts_value(mw: u32) -> String {
    if mw.is_multiple_of(1000) {
        format!("{}", mw / 1000)
    } else {
        format!("{:.1}", mw as f64 / 1000.0)
    }
}

fn config_home() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(dir);
    }
    user_home()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
}

pub fn default_config_path() -> PathBuf {
    config_home().join("ryzen-power/config.conf")
}

fn legacy_config_path() -> PathBuf {
    config_home().join("ryzen_power.conf")
}

fn user_home() -> Option<PathBuf> {
    // When the program is started through sudo, use the original user's config.
    if unsafe { libc::geteuid() } == 0
        && let Ok(user) = env::var("SUDO_USER")
        && let Some(home) = home_from_passwd(&user)
    {
        return Some(home);
    }
    env::var_os("HOME").map(PathBuf::from)
}

fn home_from_passwd(user: &str) -> Option<PathBuf> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 6 && fields[0] == user {
            return Some(PathBuf::from(fields[5]));
        }
    }
    None
}

pub fn prepare_theme_dir(config_path: &Path) -> Result<PathBuf> {
    let themes_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("themes");
    fs::create_dir_all(themes_dir.join("default")).with_context(|| {
        format!(
            "Could not create theme directory {}",
            themes_dir.join("default").display()
        )
    })?;

    let readme = themes_dir.join("README.txt");
    if !readme.exists() {
        fs::write(&readme, THEMES_README)
            .with_context(|| format!("Could not write {}", readme.display()))?;
    }

    Ok(themes_dir)
}

impl Config {
    /// Load the config, migrating the old location and creating defaults when needed.
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let explicit_path = path.is_some();
        let path = path
            .map(Path::to_path_buf)
            .unwrap_or_else(default_config_path);

        if !explicit_path && !path.exists() {
            let legacy = legacy_config_path();
            if legacy.exists() {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).with_context(|| {
                        format!("Could not create directory {}", parent.display())
                    })?;
                }
                match fs::rename(&legacy, &path) {
                    Ok(()) => eprintln!("Moved config: {} -> {}", legacy.display(), path.display()),
                    Err(_) => {
                        fs::copy(&legacy, &path).with_context(|| {
                            format!("Could not move {} to {}", legacy.display(), path.display())
                        })?;
                        let _ = fs::remove_file(&legacy);
                        eprintln!("Moved config: {} -> {}", legacy.display(), path.display());
                    }
                }
            }
        }

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Could not create directory {}", parent.display()))?;
            }
            fs::write(&path, DEFAULT_CONFIG)
                .with_context(|| format!("Could not write {}", path.display()))?;
            eprintln!("Created config: {}", path.display());
        }

        prepare_theme_dir(&path)?;

        let text = fs::read_to_string(&path)
            .with_context(|| format!("Could not read {}", path.display()))?;
        Self::parse(path, &text)
    }

    pub fn parse(path: PathBuf, text: &str) -> Result<Self> {
        let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();

        for (line_no, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if line.starts_with('[') {
                if !line.ends_with(']') {
                    bail!(
                        "{}:{}: unterminated section: {}",
                        path.display(),
                        line_no + 1,
                        line
                    );
                }
                let name = line[1..line.len() - 1].trim().to_lowercase();
                if name.is_empty() {
                    bail!("{}:{}: empty section name", path.display(), line_no + 1);
                }
                if sections.iter().any(|(existing, _)| existing == &name) {
                    bail!(
                        "{}:{}: duplicate section [{}]",
                        path.display(),
                        line_no + 1,
                        name
                    );
                }
                sections.push((name, Vec::new()));
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                bail!(
                    "{}:{}: expected key = value: {}",
                    path.display(),
                    line_no + 1,
                    line
                );
            };

            let Some((_, values)) = sections.last_mut() else {
                bail!(
                    "{}:{}: key outside a section: {}",
                    path.display(),
                    line_no + 1,
                    line
                );
            };

            values.push((key.trim().to_ascii_lowercase(), value.trim().to_string()));
        }

        if sections.is_empty() {
            bail!("{}: config is empty", path.display());
        }

        let mut general = General::default();
        let mut profiles = Vec::new();
        let mut warnings = Vec::new();
        let mut color_index = 0usize;

        for (section, values) in &sections {
            if section == "general" {
                parse_general(values, &mut general, &path)?;
                continue;
            }

            let profile = parse_profile(section, values, color_index, &path, &mut warnings)?;
            color_index += 1;
            profiles.push(profile);
        }

        if profiles.is_empty() {
            bail!("{}: config contains no profiles", path.display());
        }

        if let Some(default) = &general.default_profile
            && resolve_profile(&profiles, default).is_none()
        {
            warnings.push(format!(
                "default profile '{default}' was not found in the config"
            ));
        }

        for warning in warnings {
            eprintln!("Warning: {warning}");
        }

        let themes_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("themes");

        Ok(Self {
            path,
            themes_dir,
            general,
            profiles,
        })
    }

    pub fn resolve_profile(&self, name: &str) -> Option<&Profile> {
        resolve_profile(&self.profiles, name)
    }

    pub fn default_profile(&self) -> Option<&Profile> {
        self.general
            .default_profile
            .as_deref()
            .and_then(|name| self.resolve_profile(name))
    }

    /// File that remembers the last profile applied by ryzen-power.
    /// Lives next to the config: `config.conf` -> `config.state`.
    pub fn state_path(&self) -> PathBuf {
        self.path.with_extension("state")
    }

    /// Name of the last profile applied by ryzen-power, if any.
    pub fn last_applied_profile(&self) -> Option<String> {
        let text = fs::read_to_string(self.state_path()).ok()?;
        let name = text.trim();
        (!name.is_empty()).then(|| name.to_string())
    }

    /// Remember `name` as the last applied profile so `--next` can continue
    /// the cycle from it.
    pub fn record_applied_profile(&self, name: &str) -> Result<()> {
        let path = self.state_path();
        fs::write(&path, format!("{name}\n"))
            .with_context(|| format!("Could not write {}", path.display()))
    }
}

fn parse_general(values: &[(String, String)], general: &mut General, path: &Path) -> Result<()> {
    for (key, value) in values {
        match key.as_str() {
            "default" => {
                general.default_profile = if value.is_empty() {
                    None
                } else {
                    Some(value.to_lowercase())
                };
            }
            "theme" => general.theme = parse_theme(value, path)?,
            "osd" => general.osd = parse_bool(value, path, key)?,
            "osd_timeout_ms" => {
                let v = parse_u32(value, path, key)?;
                if !(300..=10_000).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 300 and 10000 ms",
                        path.display()
                    );
                }
                general.osd_timeout_ms = u64::from(v);
            }
            "osd_position" => {
                general.osd_position = match value.trim().to_ascii_lowercase().as_str() {
                    "bottom" => OsdPosition::Bottom,
                    "center" => OsdPosition::Center,
                    "top" => OsdPosition::Top,
                    other => bail!(
                        "{}: general.osd_position: expected bottom, center or top, got '{other}'",
                        path.display()
                    ),
                };
            }
            "osd_margin" | "osd_bottom_margin" => {
                let v = parse_u32(value, path, key)?;
                if v > 2000 {
                    bail!("{}: general.{key} must not exceed 2000 px", path.display());
                }
                general.osd_margin = v as i32;
            }
            "osd_width" => {
                let v = parse_u32(value, path, key)?;
                if !(200..=1600).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 200 and 1600 px",
                        path.display()
                    );
                }
                general.osd_width = v as i32;
            }
            "background_color" => {
                general.background_color = parse_color(value, path, &format!("general.{key}"))?;
            }
            "background_opacity" => {
                let v = parse_f64(value, path, key)?;
                if !(0.0..=1.0).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 0.0 and 1.0",
                        path.display()
                    );
                }
                general.background_opacity = v;
            }
            "border_color" => {
                general.border_color = parse_color(value, path, &format!("general.{key}"))?;
            }
            "border_opacity" => {
                let v = parse_f64(value, path, key)?;
                if !(0.0..=1.0).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 0.0 and 1.0",
                        path.display()
                    );
                }
                general.border_opacity = v;
            }
            "border_width" => {
                let v = parse_u32(value, path, key)?;
                if v > 16 {
                    bail!("{}: general.{key} must not exceed 16 px", path.display());
                }
                general.border_width = v as i32;
            }
            "border_radius" => {
                let v = parse_u32(value, path, key)?;
                if v > 48 {
                    bail!("{}: general.{key} must not exceed 48 px", path.display());
                }
                general.border_radius = v as i32;
            }
            "padding_x" => {
                let v = parse_u32(value, path, key)?;
                if v > 64 {
                    bail!("{}: general.{key} must not exceed 64 px", path.display());
                }
                general.padding_x = v as i32;
            }
            "padding_y" => {
                let v = parse_u32(value, path, key)?;
                if v > 64 {
                    bail!("{}: general.{key} must not exceed 64 px", path.display());
                }
                general.padding_y = v as i32;
            }
            "font_family" => {
                general.font_family = value.trim().to_string();
            }
            "text_color" => {
                general.text_color = parse_color(value, path, &format!("general.{key}"))?;
            }
            "subtitle_color" => {
                let trimmed = value.trim();
                general.subtitle_color = if trimmed.is_empty() {
                    None
                } else {
                    Some(parse_color(trimmed, path, &format!("general.{key}"))?)
                };
            }
            "subtitle_opacity" => {
                let v = parse_f64(value, path, key)?;
                if !(0.0..=1.0).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 0.0 and 1.0",
                        path.display()
                    );
                }
                general.subtitle_opacity = v;
            }
            "badge_color" => {
                let trimmed = value.trim();
                general.badge_color = if trimmed.is_empty() {
                    None
                } else {
                    Some(parse_color(trimmed, path, &format!("general.{key}"))?)
                };
            }
            "icon_position" => {
                general.icon_position = match value.trim().to_ascii_lowercase().as_str() {
                    "left" => IconPosition::Left,
                    "right" => IconPosition::Right,
                    "top" => IconPosition::Top,
                    "none" => IconPosition::None,
                    other => bail!(
                        "{}: general.icon_position: expected left, right, top or none, got '{other}'",
                        path.display()
                    ),
                };
            }
            "icon_size" => {
                let v = parse_u32(value, path, key)?;
                if !(12..=96).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 12 and 96 px",
                        path.display()
                    );
                }
                general.icon_size = v as i32;
            }
            "icon_background" => {
                general.icon_background = match value.trim().to_ascii_lowercase().as_str() {
                    "circle" => IconBackground::Circle,
                    "rounded" => IconBackground::Rounded,
                    "none" => IconBackground::None,
                    other => bail!(
                        "{}: general.icon_background: expected circle, rounded or none, got '{other}'",
                        path.display()
                    ),
                };
            }
            "icon_padding" => {
                let v = parse_u32(value, path, key)?;
                if v > 32 {
                    bail!("{}: general.{key} must not exceed 32 px", path.display());
                }
                general.icon_padding = v as i32;
            }
            "show_badge" => general.show_badge = parse_bool(value, path, key)?,
            "show_subtitle" => general.show_subtitle = parse_bool(value, path, key)?,
            "show_bar" => general.show_bar = parse_bool(value, path, key)?,
            "bar_height" => {
                let v = parse_u32(value, path, key)?;
                if !(2..=24).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 2 and 24 px",
                        path.display()
                    );
                }
                general.bar_height = v as i32;
            }
            "bar_position" => {
                general.bar_position = match value.trim().to_ascii_lowercase().as_str() {
                    "bottom" => BarPosition::Bottom,
                    "top" => BarPosition::Top,
                    other => bail!(
                        "{}: general.bar_position: expected bottom or top, got '{other}'",
                        path.display()
                    ),
                };
            }
            "text_align" => {
                general.text_align = match value.trim().to_ascii_lowercase().as_str() {
                    "left" => TextAlign::Left,
                    "center" => TextAlign::Center,
                    other => bail!(
                        "{}: general.text_align: expected left or center, got '{other}'",
                        path.display()
                    ),
                };
            }
            "title_size" => {
                let v = parse_u32(value, path, key)?;
                if !(8..=48).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 8 and 48 px",
                        path.display()
                    );
                }
                general.title_size = v as i32;
            }
            "subtitle_size" => {
                let v = parse_u32(value, path, key)?;
                if !(8..=48).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 8 and 48 px",
                        path.display()
                    );
                }
                general.subtitle_size = v as i32;
            }
            "badge_size" => {
                let v = parse_u32(value, path, key)?;
                if !(8..=48).contains(&v) {
                    bail!(
                        "{}: general.{key} must be between 8 and 48 px",
                        path.display()
                    );
                }
                general.badge_size = v as i32;
            }
            "suggest_polkit" => general.suggest_polkit = parse_bool(value, path, key)?,
            other => bail!("{}: unknown key general.{other}", path.display()),
        }
    }
    Ok(())
}

fn parse_profile(
    name: &str,
    values: &[(String, String)],
    index: usize,
    path: &Path,
    warnings: &mut Vec<String>,
) -> Result<Profile> {
    let mut label = None;
    let mut icon = None;
    let mut color = None;
    let mut limits = Limits::default();

    for (key, value) in values {
        match key.as_str() {
            "label" => label = Some(value.clone()),
            "icon" => icon = Some(value.clone()),
            "color" => {
                color = Some(Rgb::parse(value).with_context(|| {
                    format!(
                        "{}: [{name}].color: expected #rgb or #rrggbb, got '{value}'",
                        path.display()
                    )
                })?)
            }
            "stapm" => limits.stapm = Some(parse_power(value, path, name, key)?),
            "slow" => limits.slow = Some(parse_power(value, path, name, key)?),
            "fast" => limits.fast = Some(parse_power(value, path, name, key)?),
            "apu_slow_limit" => limits.apu_slow_limit = Some(parse_power(value, path, name, key)?),
            "skin_temp_limit" => {
                limits.skin_temp_limit = Some(parse_power(value, path, name, key)?)
            }
            "tctl" => limits.tctl = Some(parse_u32(value, path, key)?),
            "apu_skin_temp" => limits.apu_skin_temp = Some(parse_u32(value, path, key)?),
            "dgpu_skin_temp" => limits.dgpu_skin_temp = Some(parse_u32(value, path, key)?),
            "slow_time" => limits.slow_time = Some(parse_u32(value, path, key)?),
            "stapm_time" => limits.stapm_time = Some(parse_u32(value, path, key)?),
            "tdc" | "vrm-current" => limits.tdc = Some(parse_current(value, path, name, key)?),
            "tdc_soc" | "vrmsoc-current" => {
                limits.tdc_soc = Some(parse_current(value, path, name, key)?)
            }
            "vrmgfx-current" | "vrmgfx_current" => {
                limits.vrmgfx_current = Some(parse_current(value, path, name, key)?)
            }
            "vrmcvip-current" | "vrmcvip_current" => {
                limits.vrmcvip_current = Some(parse_current(value, path, name, key)?)
            }
            "edc" | "vrmmax-current" => limits.edc = Some(parse_current(value, path, name, key)?),
            "edc_soc" | "vrmsocmax-current" => {
                limits.edc_soc = Some(parse_current(value, path, name, key)?)
            }
            "vrmgfxmax-current" | "vrmgfxmax_current" => {
                limits.vrmgfxmax_current = Some(parse_current(value, path, name, key)?)
            }
            "psi0-current" | "psi0_current" => {
                limits.psi0_current = Some(parse_current(value, path, name, key)?)
            }
            "psi3cpu-current" | "psi3cpu_current" => {
                limits.psi3cpu_current = Some(parse_current(value, path, name, key)?)
            }
            "psi0soc-current" | "psi0soc_current" => {
                limits.psi0soc_current = Some(parse_current(value, path, name, key)?)
            }
            "psi3gfx-current" | "psi3gfx_current" => {
                limits.psi3gfx_current = Some(parse_current(value, path, name, key)?)
            }
            "max-socclk-frequency" | "max_socclk_frequency" => {
                limits.max_socclk_frequency = Some(parse_u32(value, path, key)?)
            }
            "min-socclk-frequency" | "min_socclk_frequency" => {
                limits.min_socclk_frequency = Some(parse_u32(value, path, key)?)
            }
            "max-fclk-frequency" | "max_fclk_frequency" => {
                limits.max_fclk_frequency = Some(parse_u32(value, path, key)?)
            }
            "min-fclk-frequency" | "min_fclk_frequency" => {
                limits.min_fclk_frequency = Some(parse_u32(value, path, key)?)
            }
            "max-vcn" | "max_vcn" => limits.max_vcn = Some(parse_u32(value, path, key)?),
            "min-vcn" | "min_vcn" => limits.min_vcn = Some(parse_u32(value, path, key)?),
            "max-lclk" | "max_lclk" => limits.max_lclk = Some(parse_u32(value, path, key)?),
            "min-lclk" | "min_lclk" => limits.min_lclk = Some(parse_u32(value, path, key)?),
            "max-gfxclk" | "max_gfxclk" => {
                limits.max_gfxclk = Some(parse_u32(value, path, key)?)
            }
            "min-gfxclk" | "min_gfxclk" => {
                limits.min_gfxclk = Some(parse_u32(value, path, key)?)
            }
            "prochot-deassertion-ramp" | "prochot_deassertion_ramp" => {
                limits.prochot_deassertion_ramp = Some(parse_u32(value, path, key)?)
            }
            "gfx-clk" | "gfx_clk" => limits.gfx_clk = Some(parse_u32(value, path, key)?),
            "oc-clk" | "oc_clk" => limits.oc_clk = Some(parse_u32(value, path, key)?),
            "oc-volt" | "oc_volt" => limits.oc_volt = Some(parse_u32(value, path, key)?),
            "enable-oc" | "enable_oc" => {
                limits.enable_oc = Some(parse_bool(value, path, key)?)
            }
            "disable-oc" | "disable_oc" => {
                limits.disable_oc = Some(parse_bool(value, path, key)?)
            }
            "set-coall" | "set_coall" | "coall" => {
                limits.set_coall = Some(parse_u32(value, path, key)?)
            }
            "set-coper" | "set_coper" | "coper" => {
                limits.set_coper = Some(parse_u32(value, path, key)?)
            }
            "set-cogfx" | "set_cogfx" | "cogfx" => {
                limits.set_cogfx = Some(parse_u32(value, path, key)?)
            }
            "power-saving" | "power_saving" => {
                limits.power_saving = Some(parse_bool(value, path, key)?)
            }
            "max-performance" | "max_performance" => {
                limits.max_performance = Some(parse_bool(value, path, key)?)
            }
            other => warnings.push(format!("[{name}]: unknown key '{other}' was ignored")),
        }
    }

    if limits.is_empty() {
        bail!("{}: profile [{name}] has no power limits", path.display());
    }

    if let (Some(slow), Some(fast)) = (limits.slow, limits.fast)
        && fast < slow
    {
        warnings.push(format!(
            "[{name}]: fast ({}) is lower than slow ({}), which is unusual",
            watts(fast),
            watts(slow)
        ));
    }
    if let (Some(stapm), Some(slow)) = (limits.stapm, limits.slow)
        && slow < stapm
    {
        warnings.push(format!(
            "[{name}]: slow ({}) is lower than stapm ({}), which is unusual",
            watts(slow),
            watts(stapm)
        ));
    }

    let label = label.unwrap_or_else(|| name.to_string());
    let icon = icon.unwrap_or_else(|| default_icon(name).to_string());
    let color = color.unwrap_or_else(|| palette()[index % palette().len()]);

    Ok(Profile {
        name: name.to_string(),
        label,
        icon,
        color,
        limits,
    })
}

fn default_icon(name: &str) -> &'static str {
    if name.contains("eco") || name.contains("save") || name.contains("quiet") {
        "power-profile-power-saver-symbolic"
    } else if name.contains("perf") || name.contains("max") || name.contains("boost") {
        "power-profile-performance-symbolic"
    } else {
        "power-profile-balanced-symbolic"
    }
}

fn palette() -> [Rgb; 6] {
    [
        Rgb(0x4c, 0xcb, 0x74),
        Rgb(0x58, 0xa6, 0xff),
        Rgb(0xff, 0x9d, 0x4d),
        Rgb(0xc0, 0x84, 0xfc),
        Rgb(0xf4, 0x72, 0xb6),
        Rgb(0x2d, 0xd4, 0xbf),
    ]
}

fn resolve_profile<'a>(profiles: &'a [Profile], name: &str) -> Option<&'a Profile> {
    let name = name.to_lowercase();
    if let Some(profile) = profiles.iter().find(|profile| profile.name == name) {
        return Some(profile);
    }

    let alias = match name.as_str() {
        "balance" | "normal" => "balanced",
        "save" | "powersave" | "power-save" | "power_saving" => "eco",
        "perf" | "max" | "performance" => "performance",
        _ => return None,
    };
    profiles.iter().find(|profile| profile.name == alias)
}

fn parse_theme(value: &str, path: &Path) -> Result<String> {
    let theme = value.trim();
    if theme.is_empty() {
        bail!(
            "{}: general.theme: theme name cannot be empty",
            path.display()
        );
    }
    if theme == "."
        || theme == ".."
        || theme.contains('/')
        || theme.contains('\\')
        || theme.contains('\0')
    {
        bail!(
            "{}: general.theme: expected a simple directory name, got '{value}'",
            path.display()
        );
    }
    Ok(theme.to_string())
}

fn parse_bool(value: &str, path: &Path, key: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!(
            "{}: {key}: expected true or false, got '{value}'",
            path.display()
        ),
    }
}

fn parse_u32(value: &str, path: &Path, key: &str) -> Result<u32> {
    let cleaned = value.trim().replace('_', "");
    let parsed = if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        u32::from_str_radix(hex, 16)
    } else {
        cleaned.parse::<u32>()
    };
    parsed.with_context(|| {
        format!(
            "{}: {key}: expected an integer, got '{value}'",
            path.display()
        )
    })
}

fn parse_f64(value: &str, path: &Path, key: &str) -> Result<f64> {
    value.trim().parse::<f64>().with_context(|| {
        format!(
            "{}: {key}: expected a number, got '{value}'",
            path.display()
        )
    })
}

fn parse_color(value: &str, path: &Path, key: &str) -> Result<Rgb> {
    Rgb::parse(value).with_context(|| {
        format!(
            "{}: {key}: expected #rgb or #rrggbb, got '{value}'",
            path.display()
        )
    })
}

fn parse_power(value: &str, path: &Path, section: &str, key: &str) -> Result<u32> {
    let raw = value.trim().replace('_', "");
    let lower = raw.to_ascii_lowercase();

    let mw = if let Some(number) = lower.strip_suffix("mw") {
        parse_number(number, path, section, key, value)?
    } else if let Some(number) = lower.strip_suffix('w') {
        parse_number(number, path, section, key, value)? * 1000.0
    } else {
        parse_number(&lower, path, section, key, value)?
    };

    let mw = mw.round();
    if !(0.0..=f64::from(u32::MAX)).contains(&mw) {
        bail!(
            "{}: [{section}].{key}: invalid power value '{value}'",
            path.display()
        );
    }
    Ok(mw as u32)
}

fn parse_current(value: &str, path: &Path, section: &str, key: &str) -> Result<u32> {
    let raw = value.trim().replace('_', "");
    let lower = raw.to_ascii_lowercase();

    let ma = if let Some(number) = lower.strip_suffix("ma") {
        parse_number(number, path, section, key, value)?
    } else if let Some(number) = lower.strip_suffix('a') {
        parse_number(number, path, section, key, value)? * 1000.0
    } else {
        parse_number(&lower, path, section, key, value)?
    };

    let ma = ma.round();
    if !(0.0..=f64::from(u32::MAX)).contains(&ma) {
        bail!(
            "{}: [{section}].{key}: invalid current value '{value}'",
            path.display()
        );
    }
    Ok(ma as u32)
}

fn parse_number(value: &str, path: &Path, section: &str, key: &str, original: &str) -> Result<f64> {
    value.trim().parse::<f64>().with_context(|| {
        format!(
            "{}: [{section}].{key}: expected a number, got '{original}'",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Config {
        Config::parse(PathBuf::from("test.conf"), text).unwrap()
    }

    #[test]
    fn parses_default_config() {
        let config = parse(DEFAULT_CONFIG);
        assert_eq!(config.profiles.len(), 3);
        assert_eq!(config.profiles[0].name, "eco");
        assert_eq!(config.profiles[0].limits.stapm, Some(25_000));
        assert_eq!(config.profiles[1].limits.fast, Some(62_000));
        assert_eq!(
            config
                .default_profile()
                .map(|profile| profile.name.as_str()),
            Some("balanced")
        );
        assert_eq!(config.general.theme, "default");
        assert_eq!(config.general.icon_position, IconPosition::Left);
    }

    #[test]
    fn parses_watt_suffix_and_aliases() {
        let text = r#"
[general]
default = balance
theme = my-theme
icon_position = right
icon_background = none
osd_position = top
bar_position = top
text_align = center
background_opacity = 0.5
[balanced]
stapm = 45w
slow = 52_000mW
fast = 62W
tctl = 90
[eco]
stapm = 25w
slow = 30w
fast = 35w
[performance]
stapm = 50w
"#;
        let config = parse(text);
        assert_eq!(config.profiles[0].limits.stapm, Some(45_000));
        assert_eq!(config.profiles[0].limits.slow, Some(52_000));
        assert_eq!(config.profiles[0].limits.fast, Some(62_000));
        assert_eq!(config.general.theme, "my-theme");
        assert_eq!(config.general.icon_position, IconPosition::Right);
        assert_eq!(config.general.icon_background, IconBackground::None);
        assert_eq!(config.general.osd_position, OsdPosition::Top);
        assert_eq!(config.general.bar_position, BarPosition::Top);
        assert_eq!(config.general.text_align, TextAlign::Center);
        assert!((config.general.background_opacity - 0.5).abs() < f64::EPSILON);
        assert_eq!(
            config.resolve_profile("balance").map(|p| p.name.as_str()),
            Some("balanced")
        );
    }

    #[test]
    fn rejects_unsafe_theme_names() {
        for theme in ["../evil", "a/b", "a\\b", "..", ""] {
            let text = format!("[general]\ntheme = {theme}\n[eco]\nstapm = 25w\n");
            assert!(
                Config::parse(PathBuf::from("test.conf"), &text).is_err(),
                "theme {theme:?} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_profile_without_limits() {
        let text = "[general]\nosd = false\n[empty]\nlabel = Empty\n";
        assert!(Config::parse(PathBuf::from("test.conf"), text).is_err());
    }

    #[test]
    fn passes_profile_values_through_without_range_limits() {
        let text = "[general]\nosd = false\n[eco]\nstapm = 500w\nslow = 550w\nfast = 600w\ntctl = 200\ntdc = 500a\nslow_time = 999999\nstapm_time = 0\n";
        let config = parse(text);
        let limits = &config.profiles[0].limits;
        assert_eq!(limits.stapm, Some(500_000));
        assert_eq!(limits.slow, Some(550_000));
        assert_eq!(limits.fast, Some(600_000));
        assert_eq!(limits.tctl, Some(200));
        assert_eq!(limits.tdc, Some(500_000));
        assert_eq!(limits.slow_time, Some(999_999));
        assert_eq!(limits.stapm_time, Some(0));
    }

    #[test]
    fn parses_skin_temperature_limits() {
        let text =
            "[general]\nosd = false\n[eco]\nstapm = 25w\napu_skin_temp = 45\ndgpu_skin_temp = 40\n";
        let config = parse(text);
        let limits = &config.profiles[0].limits;
        assert_eq!(limits.apu_skin_temp, Some(45));
        assert_eq!(limits.dgpu_skin_temp, Some(40));
        let args = limits.ryzenadj_args();
        assert!(args.contains(&"--apu-skin-temp=45".to_string()));
        assert!(args.contains(&"--dgpu-skin-temp=40".to_string()));
        assert_eq!(
            limits.summary(),
            "STAPM 25 W · APU skin 45 °C · dGPU skin 40 °C"
        );
    }

    #[test]
    fn parses_apu_and_skin_power_limits() {
        let text = "[general]\nosd = false\n[eco]\nstapm = 25w\napu_slow_limit = 45w\nskin_temp_limit = 15000\n";
        let config = parse(text);
        let limits = &config.profiles[0].limits;
        assert_eq!(limits.apu_slow_limit, Some(45_000));
        assert_eq!(limits.skin_temp_limit, Some(15_000));
        let args = limits.ryzenadj_args();
        assert!(args.contains(&"--apu-slow-limit=45000".to_string()));
        assert!(args.contains(&"--skin-temp-limit=15000".to_string()));
        assert_eq!(
            limits.summary(),
            "STAPM 25 W · APU slow 45 W · Skin power 15 W"
        );
    }

    #[test]
    fn accepts_ryzenadj_current_option_names() {
        let text = "[general]\nosd = false\n[eco]\nstapm = 25w\nvrm-current = 50a\nvrmsoc-current = 30a\nvrmmax-current = 70a\nvrmsocmax-current = 40a\n";
        let config = parse(text);
        let limits = &config.profiles[0].limits;
        assert_eq!(limits.tdc, Some(50_000));
        assert_eq!(limits.tdc_soc, Some(30_000));
        assert_eq!(limits.edc, Some(70_000));
        assert_eq!(limits.edc_soc, Some(40_000));
        let args = limits.ryzenadj_args();
        assert!(args.contains(&"--vrm-current=50000".to_string()));
        assert!(args.contains(&"--vrmsoc-current=30000".to_string()));
        assert!(args.contains(&"--vrmmax-current=70000".to_string()));
        assert!(args.contains(&"--vrmsocmax-current=40000".to_string()));
    }

    #[test]
    fn rejects_invalid_power_values() {
        for value in ["abc", "-5w", "nan", "inf"] {
            let text = format!("[general]\nosd = false\n[eco]\nstapm = {value}\n");
            assert!(
                Config::parse(PathBuf::from("test.conf"), &text).is_err(),
                "power {value:?} should be rejected"
            );
        }
    }

    #[test]
    fn parses_colors() {
        assert_eq!(Rgb::parse("#4ccb74"), Some(Rgb(0x4c, 0xcb, 0x74)));
        assert_eq!(Rgb::parse("#fff"), Some(Rgb(255, 255, 255)));
    }

    #[test]
    fn parses_remaining_ryzenadj_options() {
        let text = r#"[general]
osd = false
[eco]
stapm = 25w
vrmgfx-current = 20a
vrmcvip-current = 10a
vrmgfxmax_current = 30a
psi0-current = 15a
psi3cpu_current = 5a
psi0soc-current = 8a
psi3gfx_current = 4a
max-socclk-frequency = 1200
min-socclk-frequency = 400
max-fclk-frequency = 1800
min-fclk-frequency = 800
max-vcn = 600
min-vcn = 300
max-lclk = 700
min-lclk = 350
max-gfxclk = 2200
min-gfxclk = 400
prochot-deassertion-ramp = 2
gfx-clk = 1600
oc-clk = 3800
oc-volt = 104
enable-oc = true
set-coall = 0x00100000
set-coper = 10
set-cogfx = 5
power-saving = true
"#;
        let config = parse(text);
        let limits = &config.profiles[0].limits;
        assert_eq!(limits.vrmgfx_current, Some(20_000));
        assert_eq!(limits.vrmgfxmax_current, Some(30_000));
        assert_eq!(limits.psi3cpu_current, Some(5_000));
        assert_eq!(limits.max_socclk_frequency, Some(1200));
        assert_eq!(limits.min_gfxclk, Some(400));
        assert_eq!(limits.prochot_deassertion_ramp, Some(2));
        assert_eq!(limits.oc_clk, Some(3800));
        assert_eq!(limits.enable_oc, Some(true));
        assert_eq!(limits.power_saving, Some(true));
        let args = limits.ryzenadj_args();
        assert!(args.contains(&"--vrmgfx-current=20000".to_string()));
        assert!(args.contains(&"--vrmgfxmax_current=30000".to_string()));
        assert!(args.contains(&"--psi3cpu_current=5000".to_string()));
        assert!(args.contains(&"--psi3gfx_current=4000".to_string()));
        assert!(args.contains(&"--max-socclk-frequency=1200".to_string()));
        assert!(args.contains(&"--prochot-deassertion-ramp=2".to_string()));
        assert!(args.contains(&"--gfx-clk=1600".to_string()));
        assert!(args.contains(&"--oc-clk=3800".to_string()));
        assert!(args.contains(&"--oc-volt=104".to_string()));
        assert!(args.contains(&"--enable-oc".to_string()));
        assert!(args.contains(&"--set-coall=1048576".to_string()));
        assert!(args.contains(&"--set-coper=10".to_string()));
        assert!(args.contains(&"--set-cogfx=5".to_string()));
        assert!(args.contains(&"--power-saving".to_string()));
    }

    #[test]
    fn accepts_flag_only_profile() {
        let text = "[general]\nosd = false\n[saver]\nmax-performance = true\n";
        let config = parse(text);
        let limits = &config.profiles[0].limits;
        assert_eq!(limits.max_performance, Some(true));
        assert_eq!(limits.ryzenadj_args(), vec!["--max-performance".to_string()]);
    }
}
