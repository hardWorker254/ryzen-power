# ryzen-power

AMD Ryzen power-limit profile switcher with a translucent on-screen display, inspired by the Windows volume indicator.

![OSD](docs/osd.png)

Run one command, and ryzen-power reads a profile from `~/.config/ryzen-power/config.conf`, applies its limits through `ryzenadj`, and shows a compact OSD with the profile name, limits, and scale.

Tested on an **AMD Ryzen 7 PRO 8845HS** with Artix Linux + Hyprland.

## Features

- Reads profiles from `~/.config/ryzen-power/config.conf`.
- Changes `STAPM`, `PPT slow`, `PPT fast`, `Tctl`, and optional current/time limits through `ryzenadj`.
- Shows a translucent OSD with the profile label, active limits, and a segmented scale.
- Uses custom theme images from `~/.config/ryzen-power/themes/<theme>/<profile>.img`.
- Falls back to the default theme, the profile icon, and then to built-in glyphs.
- Extensive OSD layout controls: position, margin, width, icon placement/size/background, text alignment, bar position/height, font sizes, opacity, and corner radius.
- Uses `pkexec` when needed, and can install a one-time polkit rule to avoid password prompts.
- Short-lived and CLI-only: apply a profile, show the OSD, exit. No daemon or tray.

## Requirements

- Linux.
- A Wayland compositor with layer-shell support (Hyprland, Sway, river, etc.) for edge-positioned overlays. The OSD also works on X11, but as a normal window.
- `ryzenadj` and the `ryzen_smu` kernel module.
- GTK 4 and `gtk4-layer-shell`.
- Rust 1.92+ to build.
- polkit and an authentication agent, or the optional polkit rule described below.

On Artix/Arch:

```bash
sudo pacman -S rust gtk4 gtk4-layer-shell polkit ryzenadj
# ryzen_smu-dkms-git is usually installed from the AUR
```

## Build and install

```bash
cd ryzen-power
./install.sh
```

The script builds a release binary and installs it to `~/.local/bin/ryzen-power`.

Manual build:

```bash
cargo build --release
install -Dm755 target/release/ryzen-power ~/.local/bin/ryzen-power
```

### Debian/Ubuntu package

```bash
./build-deb.sh
sudo apt install ./dist/ryzen-power_0.1.0-1_amd64.deb
```

The build script uses `dpkg-deb` when available and falls back to manual `ar`/`tar` assembly. Install dependencies first:

```bash
sudo apt install build-essential pkg-config libgtk-4-dev libgtk4-layer-shell-dev \
                 polkitd ryzenadj
```

### Arch/Artix package

```bash
./build-pacman.sh
sudo pacman -U dist/pacman-build/ryzen-power-0.1.0-1-x86_64.pkg.tar.zst
```

The script stages a clean source tarball under `dist/pacman-build/`, writes its checksum into `PKGBUILD`, and runs `makepkg -f`. `makepkg` requires the `base-devel` group and the dependencies listed above.

## First run

```bash
ryzen-power list
```

On first run, the config is created at `~/.config/ryzen-power/config.conf` with three profiles. The `themes/` directory and a short theme README are created next to it. If an old `~/.config/ryzen_power.conf` exists, it is migrated automatically.

Check the currently active limits:

```bash
ryzen-power status
```

## Usage

```bash
ryzen-power eco            # power saver
ryzen-power balanced       # balanced
ryzen-power performance    # performance
ryzen-power preview eco    # show the OSD without changing limits
```

Running without a profile applies the `default` from the `[general]` section.

Options:

```bash
ryzen-power --dry-run balanced   # print the ryzenadj command without running it
ryzen-power --no-osd eco         # apply without showing the OSD
ryzen-power --time 3 balanced    # show the OSD for 3 seconds
ryzen-power --config FILE eco    # use another config file
ryzen-power preview balanced --time 3
```

`--time SECONDS` overrides `osd_timeout_ms` for that run and accepts a positive whole number of seconds.

Inspect the command that would be executed:

```bash
ryzen-power --dry-run balanced
# pkexec /usr/bin/ryzenadj --stapm-limit=45000 --slow-limit=52000 --fast-limit=62000 --tctl-temp=90
```

## Hyprland keybinds

```conf
bind = SUPER, F1, exec, ryzen-power eco
bind = SUPER, F2, exec, ryzen-power balanced
bind = SUPER, F3, exec, ryzen-power performance
```

If `~/.local/bin` is not in Hyprland's `PATH`, use the full path to the binary.

## Root privileges and polkit

By default, ryzen-power first tries to run `ryzenadj` directly. If permission is denied, it retries through `pkexec` and shows the system password prompt.

On minimal Wayland setups, a polkit authentication agent may not be running. In that case `pkexec` reports that no agent was found. There are two options:

1. Install and start an agent such as `hyprpolkitagent` or `polkit-gnome`.
2. Install the polkit rule once:

```bash
sudo ryzen-power install-polkit
```

If `sudo` cannot find a binary from `~/.local/bin`, use the full path:

```bash
sudo "$(command -v ryzen-power)" install-polkit
```

The rule allows members of the `wheel` and `sudo` groups to run **only `ryzenadj`** through `pkexec` without a password. It installs two files:

- `/usr/share/polkit-1/actions/org.ryzenpower.apply.policy`
- `/etc/polkit-1/rules.d/49-ryzen-power.rules`

Remove the rule with:

```bash
sudo rm /usr/share/polkit-1/actions/org.ryzenpower.apply.policy \
        /etc/polkit-1/rules.d/49-ryzen-power.rules
```

## Configuration

The format is INI. The default path is `~/.config/ryzen-power/config.conf` (or `$XDG_CONFIG_HOME/ryzen-power/config.conf`).

```ini
[general]
default = balanced
theme = default

osd = true
osd_position = bottom
osd_margin = 110
osd_width = 380
osd_timeout_ms = 1400

background_opacity = 0.88
border_radius = 18

icon_position = left
icon_size = 24
icon_background = circle
icon_padding = 9

show_badge = true
show_subtitle = true
show_bar = true
bar_height = 8
bar_position = bottom
text_align = left

title_size = 16
subtitle_size = 12
badge_size = 18

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
```

### Profile keys

| Key | Meaning | Units |
|---|---|---|
| `stapm` | Sustained Power Limit | mW (`45000`) or W (`45w`) |
| `slow` | PPT slow / Average Power Limit | mW or W |
| `fast` | PPT fast / Actual Power Limit | mW or W |
| `tctl` | Tctl temperature limit | °C |
| `tdc` | TDC LIMIT VDD | mA or A |
| `edc` | EDC LIMIT VDD | mA or A |
| `tdc_soc` | TDC LIMIT SoC | mA or A |
| `edc_soc` | EDC LIMIT SoC | mA or A |
| `slow_time` | Slow PPT constant time | seconds |
| `stapm_time` | STAPM constant time | seconds |
| `label` | Name shown in the OSD | string |
| `icon` | GTK symbolic icon name; built-in glyph if unavailable | string |
| `color` | Profile accent color in the OSD | `#rgb` or `#rrggbb` |

You can add your own profiles: the section name becomes the command name. Limits are validated at 1–200 W, 1–300 A, and 40–115 °C.

The OSD scale is based on `fast`: profiles are sorted by that limit, and the selected profile fills the corresponding part of the scale.

### `[general]` keys

| Key | Default | Description |
|---|---|---|
| `default` | `balanced` | Profile applied when no profile is given |
| `theme` | `default` | Theme directory under `themes/` |
| `osd` | `true` | Show the OSD |
| `osd_position` | `bottom` | `bottom`, `center`, or `top` |
| `osd_margin` | `110` | Distance from the screen edge, px (ignored for `center`) |
| `osd_width` | `380` | Minimum OSD width, px |
| `osd_timeout_ms` | `1400` | How long the OSD remains fully visible (`--time` overrides it per run) |
| `background_opacity` | `0.88` | Card background opacity, `0.0`–`1.0` |
| `border_radius` | `18` | Card corner radius, px |
| `icon_position` | `left` | `left`, `right`, `top`, or `none` |
| `icon_size` | `24` | Icon size, px |
| `icon_background` | `circle` | `circle`, `rounded`, or `none` |
| `icon_padding` | `9` | Padding around the icon, px |
| `show_badge` | `true` | Show the fast-limit badge |
| `show_subtitle` | `true` | Show the limits line |
| `show_bar` | `true` | Show the segmented scale |
| `bar_height` | `8` | Scale height, px |
| `bar_position` | `bottom` | `bottom` or `top` |
| `text_align` | `left` | `left` or `center` |
| `title_size` | `16` | Profile label font size, px |
| `subtitle_size` | `12` | Limits line font size, px |
| `badge_size` | `18` | Badge font size, px |
| `suggest_polkit` | `true` | Suggest installing the polkit rule after a password prompt |

`osd_bottom_margin` is accepted as an alias for `osd_margin`.

### Custom themes and icons

Themes live under `~/.config/ryzen-power/themes/`:

```text
~/.config/ryzen-power/
├── config.conf
└── themes/
    ├── default/
    │   └── balanced.img
    └── my-theme/
        ├── eco.img
        ├── balanced.img
        └── performance.img
```

The file name must match the profile section name. The recommended extension is `.img`; `.png`, `.jpg`, `.jpeg`, `.webp`, and `.svg` are also supported.

Select the theme in `config.conf`:

```ini
[general]
theme = my-theme
```

Icon lookup order:

1. `themes/<selected-theme>/<profile>.img` (or another supported extension)
2. `themes/default/<profile>.img`
3. The profile's `icon` GTK symbolic icon
4. A built-in glyph: a leaf, an equals sign, or a bolt

Missing or unreadable images fall through to the next option, so a theme can override only some profiles.

Transparent PNG or SVG images work best. Artwork around 64–128 px is a good starting point; ryzen-power scales it to `icon_size`. To change the icon without a background bubble:

```ini
[general]
icon_background = none
icon_size = 28
```

To put the icon above the text:

```ini
[general]
icon_position = top
text_align = center
```

After editing the theme, preview it without changing power limits:

```bash
ryzen-power preview eco
```

### Example SVG icon

```bash
mkdir -p ~/.config/ryzen-power/themes/my-theme
cat > ~/.config/ryzen-power/themes/my-theme/eco.img <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <path fill="#ffffff" d="M50 10C28 10 14 22 14 40c0 4 1 8 3 11 6-14 16-22 30-26-10 7-18 17-23 30 3 1 6 2 9 2 18 0 27-15 27-37 0-4-1-7-2-10z"/>
</svg>
SVG
```

Then set `theme = my-theme` in the config.

## How it works

1. ryzen-power reads the config and selects a profile by argument or by `default`.
2. It runs `ryzenadj` with the profile limits, falling back to `pkexec` if needed.
3. After a successful apply, it shows the OSD with the profile color, icon, limits, and scale.
4. A new OSD replaces the previous one so translucent cards do not stack.

Limits are not persisted across reboots: apply the profile again after boot. This is normal `ryzenadj` behavior.

## Security notes

- Power values are in **milliwatts** unless a `w` suffix is used. `45000` = 45 W.
- Very high limits can cause overheating, throttling, or instability. Start conservatively and monitor temperatures.
- Your laptop firmware/EC may clamp or override the values. Verify with `ryzen-power status` or `sudo ryzenadj -i`.
- The polkit rule lets members of `wheel`/`sudo` run `ryzenadj` without a password. This is administrator-level access; install it deliberately.

## Troubleshooting

**`Unable to get os_access Obj, check permission`**  
No access to `/dev/mem`. Use `pkexec`, `sudo`, or the polkit rule.

**`No authentication agent` / `Error creating textual authentication agent`**  
No polkit authentication agent is running. Install `hyprpolkitagent`/`polkit-gnome`, or run `sudo ryzen-power install-polkit`.

**`ryzenadj was not found`**  
Install the `ryzenadj` package and make sure the binary is available in `PATH` or at `/usr/bin/ryzenadj`.

**The OSD does not appear at the screen edge**  
Use a Wayland compositor with layer-shell support. On X11 the OSD is a normal window and its position is controlled by the window manager.

**The values were applied, but `ryzen-power status` shows old values**  
The EC/firmware may override the limits. Check the laptop power mode and try another profile.

## License

MIT.
