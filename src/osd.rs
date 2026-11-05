use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::config::{
    BarPosition, Config, IconBackground, IconPosition, OsdPosition, Profile, Rgb, TextAlign,
};

pub struct OsdData {
    pub title: String,
    pub subtitle: String,
    pub badge: String,
    pub icon: String,
    pub theme_icons: Vec<PathBuf>,
    pub color: Rgb,
    pub fraction: f64,
    pub segments: usize,
    pub profile_name: Option<String>,
    pub is_error: bool,
}

impl OsdData {
    pub fn for_profile(config: &Config, profile: &Profile) -> Self {
        let mut fast_limits: Vec<u32> = config
            .profiles
            .iter()
            .filter_map(|profile| profile.limits.fast)
            .collect();
        fast_limits.sort_unstable();
        fast_limits.dedup();

        let fraction = match profile.limits.fast {
            Some(fast) if !fast_limits.is_empty() => {
                let rank = fast_limits
                    .iter()
                    .position(|value| *value == fast)
                    .unwrap_or(fast_limits.len() - 1);
                (rank + 1) as f64 / fast_limits.len() as f64
            }
            _ => 0.5,
        };

        Self {
            title: profile.label.clone(),
            subtitle: profile.limits.summary(),
            badge: profile.limits.fast_watts().unwrap_or_default(),
            icon: profile.icon.clone(),
            theme_icons: find_theme_icons(config, &profile.name),
            color: profile.color,
            fraction,
            segments: fast_limits.len().clamp(1, 8),
            profile_name: Some(profile.name.clone()),
            is_error: false,
        }
    }

    pub fn error(message: &str) -> Self {
        let first_line = message
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Unknown error")
            .trim();
        let subtitle = if first_line.chars().count() > 72 {
            let mut text: String = first_line.chars().take(71).collect();
            text.push('…');
            text
        } else {
            first_line.to_string()
        };

        Self {
            title: "Error".to_string(),
            subtitle,
            badge: "!".to_string(),
            icon: "dialog-error-symbolic".to_string(),
            theme_icons: Vec::new(),
            color: Rgb(0xff, 0x5c, 0x5c),
            fraction: 0.0,
            segments: 3,
            profile_name: None,
            is_error: true,
        }
    }
}

/// Return icons in fallback order: selected theme, then the default theme.
///
/// Theme files are named after the profile section, for example
/// `themes/my-theme/eco.img`. PNG, JPEG, WebP and SVG are supported as well.
fn find_theme_icons(config: &Config, profile_name: &str) -> Vec<PathBuf> {
    const EXTENSIONS: &[&str] = &["img", "png", "jpg", "jpeg", "webp", "svg"];

    let selected = config.general.theme.trim();
    let mut themes: Vec<&str> = Vec::new();
    if !selected.eq_ignore_ascii_case("default") {
        themes.push(selected);
    }
    themes.push("default");

    let mut paths = Vec::new();
    for theme in themes {
        let dir = config.themes_dir.join(theme);
        for extension in EXTENSIONS {
            let candidate = dir.join(format!("{profile_name}.{extension}"));
            if candidate.is_file() {
                paths.push(candidate);
            }
        }
    }
    paths
}

pub fn display_available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

/// Show the OSD for `timeout`, or for `general.osd_timeout_ms` when `timeout` is `None`.
pub fn show(config: &Config, data: &OsdData, timeout: Option<Duration>) -> Result<()> {
    if !display_available() {
        bail!("No graphical session (WAYLAND_DISPLAY/DISPLAY is not set)");
    }

    gtk::init().context("Could not initialize GTK 4")?;

    let display = gtk::gdk::Display::default().context("Could not open the display")?;
    let _slot = OsdSlot::claim();

    let css = build_css(config, data);
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&css);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let general = &config.general;
    let accent = accent_class(config, data);
    let icon_theme = gtk::IconTheme::for_display(&display);

    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("osd-card");
    card.set_size_request(general.osd_width, -1);
    card.set_can_target(false);

    let icon_bubble = if general.icon_position == IconPosition::None {
        None
    } else {
        Some(build_icon_bubble(config, data, &accent, &icon_theme))
    };

    let title = gtk::Label::new(Some(&data.title));
    title.add_css_class("osd-title");
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    title.set_max_width_chars(28);
    apply_text_alignment(&title, general.text_align);

    let text_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text_box.set_hexpand(true);
    text_box.set_valign(gtk::Align::Center);
    text_box.set_can_target(false);
    text_box.append(&title);

    if general.show_subtitle && !data.subtitle.is_empty() {
        let subtitle = gtk::Label::new(Some(&data.subtitle));
        subtitle.add_css_class("osd-sub");
        subtitle.set_ellipsize(gtk::pango::EllipsizeMode::End);
        subtitle.set_max_width_chars(40);
        apply_text_alignment(&subtitle, general.text_align);
        text_box.append(&subtitle);
    }

    let badge = if general.show_badge && !data.badge.is_empty() {
        let label = gtk::Label::new(Some(&data.badge));
        label.add_css_class("osd-badge");
        label.set_valign(gtk::Align::Center);
        label.set_halign(gtk::Align::End);
        Some(label)
    } else {
        None
    };

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    row.set_can_target(false);

    if general.icon_position == IconPosition::Left
        && let Some(bubble) = icon_bubble.as_ref()
    {
        row.append(bubble);
    }
    row.append(&text_box);
    if let Some(badge) = badge.as_ref() {
        row.append(badge);
    }
    if general.icon_position == IconPosition::Right
        && let Some(bubble) = icon_bubble.as_ref()
    {
        row.append(bubble);
    }

    let content = if general.icon_position == IconPosition::Top {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        column.set_can_target(false);
        if let Some(bubble) = icon_bubble.as_ref() {
            bubble.set_halign(if general.text_align == TextAlign::Center {
                gtk::Align::Center
            } else {
                gtk::Align::Start
            });
            column.append(bubble);
        }
        column.append(&row);
        column
    } else {
        row
    };

    let bar = if general.show_bar {
        let color = data.color.as_f64();
        let fraction = data.fraction;
        let segments = data.segments;
        let bar = gtk::DrawingArea::new();
        bar.set_content_height(general.bar_height);
        bar.set_size_request(-1, general.bar_height);
        bar.set_hexpand(true);
        bar.set_can_target(false);
        bar.set_draw_func(move |_, cr, width, height| {
            draw_bar(cr, width, height, color, fraction, segments);
        });
        Some(bar)
    } else {
        None
    };

    if let Some(bar) = bar.as_ref()
        && general.bar_position == BarPosition::Top
    {
        card.append(bar);
    }
    card.append(&content);
    if let Some(bar) = bar.as_ref()
        && general.bar_position == BarPosition::Bottom
    {
        card.append(bar);
    }

    let window = gtk::Window::new();
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_focusable(false);
    window.set_can_focus(false);
    window.set_can_target(false);
    window.set_default_size(general.osd_width, -1);
    window.set_child(Some(&card));

    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_namespace(Some("ryzen-power-osd"));
        window.set_layer(Layer::Overlay);
        match general.osd_position {
            OsdPosition::Bottom => {
                window.set_anchor(Edge::Bottom, true);
                window.set_margin(Edge::Bottom, general.osd_margin);
            }
            OsdPosition::Top => {
                window.set_anchor(Edge::Top, true);
                window.set_margin(Edge::Top, general.osd_margin);
            }
            OsdPosition::Center => {
                // No anchors: the compositor centers the surface on both axes.
            }
        }
        window.set_exclusive_zone(-1);
        window.set_keyboard_mode(KeyboardMode::None);
    } else {
        // X11 or a Wayland compositor without layer-shell: show a normal
        // undecorated window. The compositor decides where to place it.
        window.set_title(Some("ryzen-power"));
    }

    card.set_opacity(0.0);
    window.present();

    let main_loop = gtk::glib::MainLoop::new(None, false);
    let loop_handle = main_loop.clone();
    let animated_card = card.clone();
    let started = Instant::now();

    let fade_in = Duration::from_millis(140);
    let hold = timeout.unwrap_or_else(|| Duration::from_millis(general.osd_timeout_ms));
    let fade_out = Duration::from_millis(220);
    let hold_end = fade_in.saturating_add(hold);
    let total = hold_end.saturating_add(fade_out);

    gtk::glib::timeout_add_local(Duration::from_millis(16), move || {
        let elapsed = started.elapsed();
        let opacity = if elapsed < fade_in {
            elapsed.as_secs_f64() / fade_in.as_secs_f64()
        } else if elapsed < hold_end {
            1.0
        } else if elapsed < total {
            1.0 - (elapsed - hold_end).as_secs_f64() / fade_out.as_secs_f64()
        } else {
            0.0
        };

        animated_card.set_opacity(opacity.clamp(0.0, 1.0));

        if elapsed >= total {
            loop_handle.quit();
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });

    main_loop.run();
    window.close();
    Ok(())
}

fn apply_text_alignment(label: &gtk::Label, align: TextAlign) {
    match align {
        TextAlign::Left => {
            label.set_halign(gtk::Align::Start);
            label.set_xalign(0.0);
        }
        TextAlign::Center => {
            label.set_halign(gtk::Align::Center);
            label.set_xalign(0.5);
        }
    }
}

fn build_icon_bubble(
    config: &Config,
    data: &OsdData,
    accent: &str,
    icon_theme: &gtk::IconTheme,
) -> gtk::Box {
    let general = &config.general;
    let bubble = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bubble.add_css_class("icon-bubble");
    bubble.set_halign(gtk::Align::Center);
    bubble.set_valign(gtk::Align::Center);
    bubble.set_can_target(false);

    match general.icon_background {
        IconBackground::Circle => {
            bubble.add_css_class("icon-circle");
            bubble.add_css_class(accent);
        }
        IconBackground::Rounded => {
            bubble.add_css_class("icon-rounded");
            bubble.add_css_class(accent);
        }
        IconBackground::None => {
            bubble.add_css_class("icon-none");
        }
    }

    if let Some(image) = data
        .theme_icons
        .iter()
        .find_map(|path| theme_image(path, general.icon_size))
    {
        bubble.append(&image);
        return bubble;
    }

    if data.is_error {
        bubble.append(&glyph_area(Glyph::Bang, general.icon_size));
        return bubble;
    }

    if !data.icon.is_empty() && icon_theme.has_icon(&data.icon) {
        let icon = gtk::Image::from_icon_name(&data.icon);
        icon.set_pixel_size(general.icon_size);
        bubble.append(&icon);
    } else {
        bubble.append(&glyph_area(glyph_for(data), general.icon_size));
    }

    bubble
}

fn theme_image(path: &Path, size: i32) -> Option<gtk::Image> {
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, size, size, true).ok()?;
    let image = gtk::Image::from_pixbuf(Some(&pixbuf));
    image.set_pixel_size(size);
    Some(image)
}

fn build_css(config: &Config, data: &OsdData) -> String {
    let general = &config.general;
    let opacity = general.background_opacity.clamp(0.0, 1.0);
    let mut css = format!(
        r#"
window {{
    background: transparent;
}}
.osd-card {{
    background-color: rgba(16, 18, 24, {opacity:.2});
    border-radius: {radius}px;
    border: 1px solid rgba(255, 255, 255, 0.10);
    padding: 13px 18px 12px 18px;
}}
.osd-title {{
    color: #f5f7fb;
    font-size: {title_size}px;
    font-weight: bold;
}}
.osd-sub {{
    color: rgba(245, 247, 251, 0.62);
    font-size: {subtitle_size}px;
}}
.osd-badge {{
    color: #f5f7fb;
    font-size: {badge_size}px;
    font-weight: bold;
}}
.icon-bubble {{
    padding: {icon_padding}px;
}}
.icon-circle {{
    border-radius: 999px;
}}
.icon-rounded {{
    border-radius: 10px;
}}
.icon-none {{
    padding: 0;
    background-color: transparent;
}}
"#,
        radius = general.border_radius,
        title_size = general.title_size,
        subtitle_size = general.subtitle_size,
        badge_size = general.badge_size,
        icon_padding = general.icon_padding,
    );

    for (index, profile) in config.profiles.iter().enumerate() {
        css.push_str(&format!(
            ".accent-{index} {{ background-color: {}; }}\n",
            profile.color.css()
        ));
    }
    css.push_str(&format!(
        ".accent-error {{ background-color: {}; }}\n",
        data.color.css()
    ));

    css
}

fn accent_class(config: &Config, data: &OsdData) -> String {
    if data.is_error {
        return "accent-error".to_string();
    }

    data.profile_name
        .as_deref()
        .and_then(|name| {
            config
                .profiles
                .iter()
                .position(|profile| profile.name == name)
        })
        .map(|index| format!("accent-{index}"))
        .unwrap_or_else(|| "accent-error".to_string())
}

#[derive(Clone, Copy)]
enum Glyph {
    Leaf,
    Balance,
    Bolt,
    Bang,
}

fn glyph_for(data: &OsdData) -> Glyph {
    let name = data
        .profile_name
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();

    if name.contains("eco") || name.contains("save") || name.contains("quiet") {
        Glyph::Leaf
    } else if name.contains("perf") || name.contains("max") || name.contains("boost") {
        Glyph::Bolt
    } else {
        Glyph::Balance
    }
}

fn glyph_area(glyph: Glyph, size: i32) -> gtk::DrawingArea {
    let size = size.max(1);
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_size_request(size, size);
    area.set_can_target(false);
    area.set_draw_func(move |_, cr, width, height| draw_glyph(cr, width, height, glyph));
    area
}

fn draw_glyph(cr: &gtk::cairo::Context, width: i32, height: i32, glyph: Glyph) {
    let width = width.max(1) as f64;
    let height = height.max(1) as f64;
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.96);

    match glyph {
        Glyph::Leaf => {
            cr.move_to(width * 0.18, height * 0.82);
            cr.curve_to(
                width * 0.12,
                height * 0.40,
                width * 0.34,
                height * 0.13,
                width * 0.84,
                height * 0.16,
            );
            cr.curve_to(
                width * 0.84,
                height * 0.60,
                width * 0.62,
                height * 0.86,
                width * 0.18,
                height * 0.82,
            );
            cr.close_path();
            cr.fill().ok();

            // A cut-out vein makes the leaf readable at small sizes.
            cr.set_operator(gtk::cairo::Operator::Clear);
            cr.set_line_width((width * 0.06).max(1.0));
            cr.set_line_cap(gtk::cairo::LineCap::Round);
            cr.move_to(width * 0.25, height * 0.75);
            cr.curve_to(
                width * 0.42,
                height * 0.62,
                width * 0.58,
                height * 0.44,
                width * 0.76,
                height * 0.24,
            );
            cr.stroke().ok();
            cr.set_operator(gtk::cairo::Operator::Over);
        }
        Glyph::Balance => {
            let bar_height = height * 0.15;
            let x = width * 0.20;
            let bar_width = width * 0.60;
            rounded_rect(
                cr,
                x,
                height * 0.30,
                bar_width,
                bar_height,
                bar_height / 2.0,
            );
            cr.fill().ok();
            rounded_rect(
                cr,
                x,
                height * 0.58,
                bar_width,
                bar_height,
                bar_height / 2.0,
            );
            cr.fill().ok();
        }
        Glyph::Bolt => {
            cr.move_to(width * 0.58, height * 0.04);
            cr.line_to(width * 0.22, height * 0.56);
            cr.line_to(width * 0.46, height * 0.56);
            cr.line_to(width * 0.38, height * 0.96);
            cr.line_to(width * 0.80, height * 0.40);
            cr.line_to(width * 0.54, height * 0.40);
            cr.close_path();
            cr.fill().ok();
        }
        Glyph::Bang => {
            let bar_width = width * 0.16;
            let bar_height = height * 0.44;
            rounded_rect(
                cr,
                (width - bar_width) / 2.0,
                height * 0.16,
                bar_width,
                bar_height,
                bar_width / 2.0,
            );
            cr.fill().ok();
            cr.arc(
                width * 0.50,
                height * 0.78,
                width * 0.09,
                0.0,
                std::f64::consts::TAU,
            );
            cr.fill().ok();
        }
    }
}

fn draw_bar(
    cr: &gtk::cairo::Context,
    width: i32,
    height: i32,
    color: (f64, f64, f64),
    fraction: f64,
    segments: usize,
) {
    let width = width.max(1) as f64;
    let height = height.max(1) as f64;
    let radius = height / 2.0;

    cr.set_source_rgba(1.0, 1.0, 1.0, 0.12);
    rounded_rect(cr, 0.0, 0.0, width, height, radius);
    cr.fill().ok();

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let fill_width = (width * fraction).max(height);
        let gradient = gtk::cairo::LinearGradient::new(0.0, 0.0, width, 0.0);
        gradient.add_color_stop_rgba(0.0, color.0, color.1, color.2, 0.95);
        gradient.add_color_stop_rgba(
            1.0,
            (color.0 * 0.82).clamp(0.0, 1.0),
            (color.1 * 0.90).clamp(0.0, 1.0),
            color.2,
            0.95,
        );
        cr.set_source(&gradient).ok();
        rounded_rect(cr, 0.0, 0.0, fill_width, height, radius);
        cr.fill().ok();

        cr.set_source_rgba(1.0, 1.0, 1.0, 0.55);
        cr.arc(
            (fill_width - radius).max(radius),
            height / 2.0,
            height * 0.14,
            0.0,
            std::f64::consts::TAU,
        );
        cr.fill().ok();
    }

    if segments > 1 && segments <= 8 {
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.20);
        cr.set_line_width(1.0);
        for index in 1..segments {
            let x = (width * index as f64 / segments as f64).round() + 0.5;
            cr.move_to(x, 0.0);
            cr.line_to(x, height);
            cr.stroke().ok();
        }
    }
}

fn rounded_rect(cr: &gtk::cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    cr.new_sub_path();
    cr.arc(
        x + width - radius,
        y + radius,
        radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    cr.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    cr.arc(
        x + radius,
        y + height - radius,
        radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cr.arc(
        x + radius,
        y + radius,
        radius,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    cr.close_path();
}

/// A tiny single-instance guard: when a new OSD appears, the previous one is
/// terminated instead of stacking several translucent cards on top of each other.
struct OsdSlot {
    path: std::path::PathBuf,
    pid: u32,
}

impl OsdSlot {
    fn claim() -> Option<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let path = dir.join(format!("ryzen-power-osd-{}.pid", unsafe {
            libc::geteuid()
        }));

        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(pid) = text.trim().parse::<u32>()
            && pid != std::process::id()
        {
            stop_previous_osd(pid);
        }

        let _ = std::fs::write(&path, std::process::id().to_string());
        Some(Self {
            path,
            pid: std::process::id(),
        })
    }
}

impl Drop for OsdSlot {
    fn drop(&mut self) {
        if let Ok(text) = std::fs::read_to_string(&self.path)
            && text.trim() == self.pid.to_string()
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn stop_previous_osd(pid: u32) {
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
    let is_ryzen_power = exe
        .as_deref()
        .and_then(std::path::Path::file_name)
        .map(|name| name.to_string_lossy() == "ryzen-power")
        .unwrap_or(false);

    if is_ryzen_power {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn theme_icons_fall_back_in_order() {
        let root =
            std::env::temp_dir().join(format!("ryzen-power-theme-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let themes = root.join("themes");
        fs::create_dir_all(themes.join("selected")).unwrap();
        fs::create_dir_all(themes.join("default")).unwrap();
        fs::write(themes.join("selected/eco.img"), b"not an image").unwrap();
        fs::write(themes.join("default/eco.svg"), b"<svg/>").unwrap();

        let text = "[general]\ntheme = selected\n[eco]\nstapm = 25w\n";
        let config = Config::parse(root.join("config.conf"), text).unwrap();
        let icons = find_theme_icons(&config, "eco");
        assert_eq!(
            icons,
            vec![
                themes.join("selected/eco.img"),
                themes.join("default/eco.svg")
            ]
        );

        fs::remove_dir_all(&root).unwrap();
    }
}
