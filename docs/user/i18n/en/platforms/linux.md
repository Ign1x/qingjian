---
title: Linux
order: 2
description: "Installing Qingjian on Linux (and FreeBSD): the fcitx edition (the Fcitx5 prebuilt package, installing Fcitx5 and Fcitx4 from source) and the Wayland native edition, your first input, updating and uninstalling, and how it differs from the desktop versions."
---

The Linux version of Qingjian comes in two editions; pick one for your desktop:

- **fcitx edition**: runs as an input method in Fcitx5 or Fcitx4, and candidates use Fcitx's own candidate panel. Use it on GNOME, KDE Plasma and similar desktops, and for X11 apps. Unless stated otherwise, this page is about the fcitx edition.
- **Wayland native edition**: Qingjian runs as the input method itself, and draws the candidate window the same way as on macOS / Windows, so themes, fonts and font sizes apply. It's for desktops such as Hyprland, sway, niri and labwc; see [Wayland native edition](#Wayland-native-edition).

Both are installed from a terminal for now; a source install can start Qingjian's background service automatically when you log in, while with the prebuilt package it has to be started by hand after each login. The fcitx edition has been verified as follows:

- For Fcitx5 there's a prebuilt package for Ubuntu 26.04; other distributions install from source. Verified with GTK4 and Qt6 apps and Firefox on the GNOME desktop (Wayland) of Ubuntu 26.04.
- For Fcitx4 there's a source install for Ubuntu 22.04, verified with GTK3 text fields.
- On FreeBSD 15.1, a Fcitx5 source install has been verified in XFCE and KDE Plasma X11 sessions and with Wayland apps under sway.

Local candidates, learning, the local sentence model and the optional AI services are supported, and the Qingjian settings app is included. For differences from macOS / Windows, see [Platform differences](differences.md).

## Fcitx5 prebuilt package (Ubuntu 26.04)

The prebuilt package is built on Ubuntu 26.04 (x86_64) and tied to the system's Fcitx5 version; on other distributions, use the source install in the next section. First install Fcitx5 and input support for applications:

```sh
sudo apt install python3 fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt6 fcitx5-config-qt
```

Download the package whose name ends in `linux-x86_64.tar.gz` from the [download page](https://qingjian.app/en/download), unpack it, and run in the unpacked folder:

```sh
./install.sh

# Start Qingjian and keep this terminal open
~/.local/bin/qingjian-linux-server
```

Then add the input method as described in [Your first input](#Your-first-input) below.

## Fcitx5 source install

First install Rust 1.96 or later and the packages needed to build. On Debian / Ubuntu:

```sh
sudo apt install cmake g++ pkg-config python3 libssl-dev \
  libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev nlohmann-json3-dev \
  fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji \
  fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt6 fcitx5-config-qt
```

The three font packages are required: without `fonts-noto-core` the settings app won't open, and without the other two the Chinese text and emoji in the interface can't be displayed. The install script lists anything missing at the start.

On FreeBSD:

```sh
pkg install bash cmake pkgconf python3 rustup-init nlohmann-json \
  fcitx5 fcitx5-gtk3 fcitx5-qt6 fcitx5-configtool
rustup-init   # install Rust
```

Enable Fcitx5 as your distribution describes; X11 apps usually need `GTK_IM_MODULE=fcitx`, `QT_IM_MODULE=fcitx` and `XMODIFIERS=@im=fcitx` set, then log in again.

In the source folder, run:

```sh
# Download and verify the full dictionaries and the local sentence model
tools/release/data-fetch.sh
apps/linux/scripts/install.sh

# Start Qingjian and keep this terminal open
~/.local/bin/qingjian-linux-server
```

- To try out the install process first, use `apps/linux/scripts/install.sh --sample --debug`: it doesn't download the full dictionaries and has only a few sample words (such as 你好).
- It installs to `~/.local` by default; `--prefix /absolute/path` changes the location.
- Install and run it as your own user, without `sudo`.

## Fcitx4 source install (Ubuntu 22.04)

If you already use Fcitx4, `fcitx-frontend-qt5` is enough for Qt5 apps. First install Rust 1.96 or later, plus:

```sh
sudo apt install cmake g++ pkg-config python3 libssl-dev fcitx-libs-dev libx11-dev libxres-dev nlohmann-json3-dev \
  fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji \
  fcitx fcitx-frontend-gtk3 fcitx-frontend-qt5 fcitx-config-gtk
```

In the source folder, run:

```sh
tools/release/data-fetch.sh
apps/linux/scripts/install.sh --fcitx4
~/.local/bin/qingjian-linux-server
```

Keep the terminal open for the last command. Run `fcitx -r` to restart Fcitx4, and add 青简 (Qingjian) to the input method list in the configuration tool.
To try out the install process first, use `apps/linux/scripts/install.sh --fcitx4 --sample --debug`.
When you switch to Qingjian, the input method icon in the tray shows the Qingjian logo.

## Your first input

1. Restart Fcitx, or log out and back in.
2. Open the Fcitx configuration tool, untick "Only Show Current Language", and add 青简 (Qingjian).
3. Switch to Qingjian, type `nihao` and press Space to get 你好.

With the Wayland native edition, Qingjian takes over the keyboard as soon as it starts and there's nothing to add; see [Wayland native edition](#Wayland-native-edition) below.

Tap `Shift` to switch between Chinese and English; for keys see the [Key reference](../getting-started/keys.md). Settings are under 青简设置 (Qingjian Settings) in the application menu.
Closing the terminal you started Qingjian from stops Qingjian; after your next login, run `~/.local/bin/qingjian-linux-server` again.

## Differences from the desktop versions

- **Candidate panel**: Fcitx's candidate panel is used and its look comes from the Fcitx theme, so Qingjian's themes, fonts, expanded horizontal layout and key hints don't apply. The panel shows only the first gloss, with part of speech and Japanese readings.
- **Chinese/English mode**: each window keeps its own; new windows start in Chinese, or can be set to start in English, see [Chinese and English](../input/english-mode.md#Default-mode-and-per-app-memory). Fcitx5's left `Shift` hotkey needs to be turned off; see the same page.
- **Tab**: goes to the app when you're not typing; while typing pinyin it accepts the sentence completion, or goes to the next page if there's none; among English candidates it picks the highlighted one.
- **Digits**: when the current page has no matching candidate, the digit is appended to the pinyin, and `hao6` followed by Enter is output as typed.
- **Direct English input**: once in it, digits and punctuation keep being appended, and ending with a space keeps the space (`gpt-6` then Space gives `gpt-6 `).
- **Sentence completion** is shown in the hint line below the candidate panel; press `Tab` to accept it.
- **Private input**: you can type in input fields Fcitx5 recognizes as private, but nothing is recorded, learned or sent; password fields are handed straight back to the app. This relies on apps marking fields correctly, and GTK4 apps' private hints currently don't come through. Fcitx4 only recognizes the password-field mark, not other private fields; if you don't want records kept, turn off learning and the input log, see [Editing the configuration file](../advanced/config-file.md#Linux).
- **Not yet supported**: Wubi and custom schemes, auxiliary code, the symbol panel, extra paging keys and highlight keys, the punctuation shortcut, automatic spacing between Chinese and Latin text.

## Wayland native edition

The Wayland native edition doesn't go through Fcitx or IBus, and Qingjian draws the candidate window itself: themes, fonts, font sizes, vertical and horizontal layouts and transition animations are the same as on macOS / Windows, and every gloss is shown beside a candidate.
Everyday typing has been verified on Hyprland and sway; behavior in individual apps hasn't been checked one by one yet.

- Desktops it works on: Hyprland, sway (1.10 and later), niri, labwc, mango and similar.
- Desktops it doesn't work on: KDE Plasma and GNOME don't offer the input method interface it needs; use the fcitx edition (Fcitx5 or IBus) there.
- It only works in Wayland apps: X11 apps running through XWayland (such as WPS) can't type Chinese with it; if you need them, use the fcitx edition.

### Turning off Fcitx5

Only one input method can take over the keyboard at a time, so the Wayland native edition can't start while Fcitx5 is running. If you use Fcitx5, do one of the following before installing:

- To stop using Fcitx5: remove the line that starts Fcitx5 from your desktop configuration (`exec-once = fcitx5 …` for Hyprland, `exec fcitx5 …` for sway, `spawn-at-startup "fcitx5" …` for niri) and the autostart entry `~/.config/autostart/org.fcitx.Fcitx5.desktop` (if present), then log in again.
- To keep Fcitx5 for X11 apps: in the Fcitx5 configuration tool, turn off "Wayland Input method frontend" under Addons, then restart Fcitx5.

### Installing

First install Rust 1.96 or later and the packages needed to build. On Debian / Ubuntu:

```sh
sudo apt install pkg-config python3 libssl-dev libxkbcommon0 \
  fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji
```

On Arch Linux:

```sh
sudo pacman -S --needed base-devel python openssl libxkbcommon noto-fonts noto-fonts-cjk noto-fonts-emoji
```

The Chinese fonts are required: Qingjian draws the candidate window itself, and without a Chinese font the candidates can't be displayed. The install script lists anything missing at the start. In the source folder, run:

```sh
tools/release/data-fetch.sh
apps/linux/scripts/install.sh --frontend wayland --enable-service
```

- `--enable-service` makes the Qingjian service start automatically when you log in, and starts the Wayland native edition right away. Without it, start it with `systemctl --user start qingjian-wayland.service`.
- If you sometimes also log in to KDE Plasma, GNOME or similar desktops and want both editions, use `--frontend auto,wayland`.
- To try out the install process only, add `--sample --debug`, as with the fcitx edition.

### Starting with the desktop

- **niri** (when you choose niri on the login screen) and desktops started with uwsm: with `--enable-service` it already starts with the desktop; otherwise run once:

  ```sh
  systemctl --user enable --now qingjian-wayland.service
  ```

- **Hyprland**: add this line to `~/.config/hypr/hyprland.conf`:

  ```text
  exec-once = systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service
  ```

- **sway**: add this line to `~/.config/sway/config`:

  ```text
  exec systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service
  ```

On other desktops, run the same command from their autostart configuration. The change takes effect after you log in again; to start it right away, run the command (the part after `exec-once =` or `exec`) in a terminal on the desktop.

### Settings in apps

- GTK apps, Qt 6.7 and later apps, Firefox, and terminals such as foot, kitty, Alacritty and WezTerm work directly.
- Don't set the environment variables `GTK_IM_MODULE` and `QT_IM_MODULE` to `fcitx` or `ibus` (leave them unset, or set them to `wayland`), or apps bypass Qingjian. They are often set in the desktop configuration (such as Hyprland's `env = QT_IM_MODULE,fcitx`), in `~/.config/environment.d/` or in `/etc/environment`; remove them and log in again.
- Chromium, Google Chrome and Electron apps (such as VS Code): add `--ozone-platform=wayland --enable-wayland-ime` to the launch options; on sway, niri and labwc also add `--wayland-text-input-version=3`.
- Apps running through XWayland have no input method.

### Differences from the fcitx edition

- Change the candidate window's theme, fonts and font sizes under 设置 → 外观 (Settings → Appearance), see [Appearance and themes](../personalize/appearance.md); with 外观 (Appearance) set to follow the system, it's shown in light mode for now.
- One Chinese/English mode is shared by all windows; switching shows no indicator, and there's no tray icon.
- English mode can't tell which app you're in yet: English candidates only appear after you untick 只在写字的应用里给 (Only in writing apps) under 设置 → 中英文 (Settings → Chinese/English).
- If you switch to another window while typing, the pinyin not yet committed is lost (the fcitx edition commits it as typed).
- Selecting text with the mouse while holding `Shift` switches between Chinese and English when you release `Shift`.
- Not yet supported: picking candidates with the mouse, the candidate right-click menu, translating selected text, and everything listed above as not yet supported in the fcitx edition.
- Run `systemctl --user status qingjian-wayland` to see whether it's running and why it stopped; if it stops working, restart it with `systemctl --user restart qingjian-wayland`.

## Updating and uninstalling

To update: close the terminal running Qingjian, run the matching install command again (for the prebuilt package, download the new package and run `./install.sh`), then restart Fcitx and start Qingjian again.
For the Wayland native edition, run `systemctl --user restart qingjian-server qingjian-wayland` after updating to switch to the new version.
The installer checks files and keeps installed files you've modified, telling you so; back them up first.

To uninstall:

```sh
apps/linux/scripts/uninstall.sh
# If you chose a custom install location:
apps/linux/scripts/uninstall.sh --prefix /absolute/path/used/when/installing
```

For the prebuilt package, use the package's `./uninstall.sh`. After uninstalling, close the terminal running Qingjian yourself and restart Fcitx. Uninstalling keeps your settings, personal dictionaries, learning data and other input methods.
Uninstalling also stops the Wayland native edition; afterwards remove the line you added for it to your desktop configuration, and if you're going back to Fcitx5, add its startup entry back.
