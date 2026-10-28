---
title: Linux
order: 2
description: 在 Linux（以及 FreeBSD）上安装青简：fcitx 版（Fcitx5 预编译包、Fcitx5 / IBus 与 Fcitx4 源码安装）与 Wayland 原生版、首次输入、更新与卸载，以及与电脑版的不同之处。
---

青简 Linux 版有两个版本，按桌面选一个：

- **fcitx 版**：作为 Fcitx5、IBus 或 Fcitx4 的一个输入法运行，候选使用输入法框架自己的候选面板。GNOME、KDE Plasma 等桌面以及 X11 应用用这个版本。本页未特别说明的内容都指 fcitx 版。
- **Wayland 原生版**：青简自己作为输入法运行，候选窗口与 macOS / Windows 一样由青简绘制，主题、字体与字号都生效。用于 Hyprland、sway、niri、labwc 等桌面，见 [Wayland 原生版](#Wayland-原生版)。

两个版本目前都需要在终端中安装；源码安装可以让青简的后台服务随登录自动启动，预编译包要每次登录后手动启动。fcitx 版的验证情况：

- Fcitx5 提供 Ubuntu 26.04 预编译包，其他发行版用源码安装。已验证 Ubuntu 26.04 的 GNOME 桌面（Wayland）上 GTK4、Qt6 应用与 Firefox。
- IBus（GNOME 自带的输入法框架）用源码安装，不用换框架。已验证按键、候选与上屏，各类应用里的表现尚未逐个验证。
- Fcitx4 提供 Ubuntu 22.04 源码安装，已验证 GTK3 文本框输入。
- FreeBSD 15.1 上用 Fcitx5 源码安装验证过 XFCE 与 KDE Plasma 的 X11 会话，以及 sway 下的 Wayland 应用。

支持本机候选、学习、本地整句模型与可选的 AI 服务，附带青简设置程序（应用菜单的「青简设置」，与 `config.toml` 是同一份配置，改完约一秒内生效，只有拼音显示位置与输入日志要重启服务）。与 macOS / Windows 的差别见 [平台差异](differences.md)。

## Fcitx5 预编译包（Ubuntu 26.04）

预编译包在 Ubuntu 26.04（x86_64）上构建，与系统的 Fcitx5 版本绑定；其他发行版请用下一节的源码安装。先装好 Fcitx5 与应用的输入支持：

```sh
sudo apt install python3 fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt6 fcitx5-config-qt
```

从[下载页](https://qingjian.app/download)下载名字以 `linux-x86_64.tar.gz` 结尾的包，解压后在解出的目录里执行：

```sh
./install.sh

# 启动青简，保持此终端开着
~/.local/bin/qingjian-linux-server
```

之后按下文[首次输入](#首次输入)添加输入法。

## Fcitx5 与 IBus 源码安装

先安装 Rust 1.96 或更新版本，再按你用的框架装编译所需的软件包。用 **Fcitx5** 时，Debian / Ubuntu 上是：

```sh
sudo apt install cmake g++ pkg-config python3 libssl-dev \
  libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev nlohmann-json3-dev \
  fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji \
  fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt6 fcitx5-config-qt
```

用 **IBus** 时不需要编译器与 Fcitx5 的开发包：

```sh
sudo apt install pkg-config python3 libssl-dev ibus \
  fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji
```

三个字体包不能省：没有 `fonts-noto-core` 设置程序打不开，缺了另外两个界面上的中文与表情显示不出来。缺了哪一项，安装脚本开头会列出来。

FreeBSD 上是：

```sh
pkg install bash cmake pkgconf python3 rustup-init nlohmann-json \
  fcitx5 fcitx5-gtk3 fcitx5-qt6 fcitx5-configtool
rustup-init   # 安装 Rust
```

按发行版的说明启用 Fcitx5；X11 应用通常需要设置 `GTK_IM_MODULE`、`QT_IM_MODULE`、`XMODIFIERS`（Fcitx5 填 `fcitx` / `@im=fcitx`，IBus 填 `ibus` / `@im=ibus`），改完重新登录。

在源码目录执行：

```sh
# 下载并校验正式词库与本地整句模型
tools/release/data-fetch.sh
apps/linux/scripts/install.sh --enable-service
```

- 安装脚本看这台机器上装了哪个框架，把对应的那份装上，并打印这次装了哪几份。两个框架都装着也没关系，它们共用同一个青简服务；只想装一边就加 `--frontend fcitx5` 或 `--frontend ibus`。
- `--enable-service` 让青简服务随登录自动启动。不加的话，要手动启动并保持终端开着：`~/.local/bin/qingjian-linux-server`。
- 只想先试一下安装流程，可用 `apps/linux/scripts/install.sh --sample --debug`，不下载正式词库，只有少量样例词（例如「你好」）。
- 缺省安装到 `~/.local`；`--prefix /绝对路径` 可改安装位置。
- 用自己的账户安装、运行，不要加 `sudo`。

## Fcitx4 源码安装（Ubuntu 22.04）

已在使用 Fcitx4 时，Qt5 应用用 `fcitx-frontend-qt5` 即可。先安装 Rust 1.96 或更新版本，以及：

```sh
sudo apt install cmake g++ pkg-config python3 libssl-dev fcitx-libs-dev libx11-dev libxres-dev nlohmann-json3-dev \
  fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji \
  fcitx fcitx-frontend-gtk3 fcitx-frontend-qt5 fcitx-config-gtk
```

在源码目录执行：

```sh
tools/release/data-fetch.sh
apps/linux/scripts/install.sh --fcitx4
~/.local/bin/qingjian-linux-server
```

最后一条命令需要保持终端开着。执行 `fcitx -r` 重启 Fcitx4，在配置工具中把「青简」加入输入法列表。
只想先试安装流程，可用 `apps/linux/scripts/install.sh --fcitx4 --sample --debug`。
切到青简时，托盘中的输入法图标显示青简标志。

## 首次输入

用 Fcitx：

1. 重启 Fcitx，或注销后重新登录。
2. 打开 Fcitx 配置工具，取消「仅显示当前语言」，添加「青简」。

用 IBus：

1. **注销后重新登录一次**。IBus 只在登录时读取输入法列表的位置，装完不重新登录看不到青简。
2. 在系统「设置 → 键盘」里添加「青简」，或执行 `ibus engine qingjian`。

用 Wayland 原生版：青简启动后直接接管键盘，不用添加输入法，见下文 [Wayland 原生版](#Wayland-原生版)。

切换到青简后输入 `nihao`，按空格得到「你好」。单击 `Shift` 切换中英，按键见 [按键速查](../getting-started/keys.md)。设置在应用菜单的「青简设置」里。
手动启动青简时，关闭那个终端会结束青简，下次登录后需再次执行 `~/.local/bin/qingjian-linux-server`；装时加了 `--enable-service` 就不用管。

## 与电脑版不同之处

- **候选面板**：使用 Fcitx / IBus 的候选面板，外观由框架的主题决定，青简的主题、字体、横排展开、键位提示不适用。面板上只显示第一条译词，带词性与日语注音。
- **中英模式**：每个窗口各记一份，新窗口从中文开始，可设为从英文开始，见 [中英文](../input/english-mode.md#默认模式与按应用记忆)。Fcitx5 的左 `Shift` 热键需要关掉，见同一页。
- **Tab**：没在打字时交给应用；打拼音时接受整句补全，没有补全时下一页；英文候选里选高亮候选。
- **数字**：当前页没有对应的候选时，数字接在拼音后面，`hao6` 回车原样输出。
- **英文直输**：进入后数字和标点继续追加，以空格结束时保留空格（`gpt-6` 后按空格得到 `gpt-6 `）。
- **整句补全**显示在候选面板下方的提示行，按 `Tab` 接受。
- **IBus 下打字中按应用的快捷键**（带 `Ctrl` / `Alt` / `Super` 且青简不用的组合，如 `Ctrl + F` 打开搜索）：已敲的字母先原样上屏到当前输入框，再交给应用。打字中用鼠标点到别的输入框时，X11 应用里拼音留在原来的框；GNOME Wayland 上没上屏的拼音会丢掉，先按 `Enter` 或选词再换框。
- **私密输入**：Fcitx5 识别为私密的输入框里可以打字，但不记录、不学习、不发送；密码框直接交还应用。这依赖应用正确标记，GTK4 应用的私密提示目前传不过来。Fcitx4 只认密码框标记，其他私密输入框认不出；不想留下记录时，可关掉学习与输入日志，见 [直接编辑配置文件](../advanced/config-file.md#Linux)。
- **暂不支持**：五笔与自制方案、符号面板、额外翻页键与高亮移动键、标点快捷键、中西文自动空格、拼音带声调。

## Wayland 原生版

Wayland 原生版不经过 Fcitx 或 IBus，候选窗口由青简绘制：主题、字体、字号、竖排与横排、过渡动画都与 macOS / Windows 相同，候选旁显示全部译词。
已在 Hyprland 与 sway 上验证日常输入，各类应用里的表现尚未逐个验证。

- 可用的桌面：Hyprland、sway（1.10 及以后）、niri、labwc、mango 等。
- 不能用的桌面：KDE Plasma 与 GNOME 不提供它需要的输入法接口，请用 fcitx 版（Fcitx5 或 IBus）。
- 只对 Wayland 应用生效：经 XWayland 运行的 X11 应用（如 WPS）打不了中文，需要它们时请用 fcitx 版。

### 关掉 Fcitx5

同一时间只能有一个输入法接管键盘，Fcitx5 运行时 Wayland 原生版无法启动。若在用 Fcitx5，安装前二选一：

- 不再使用 Fcitx5：删掉桌面配置里启动 Fcitx5 的那一行（Hyprland 的 `exec-once = fcitx5 …`、sway 的 `exec fcitx5 …`、niri 的 `spawn-at-startup "fcitx5" …`），以及自启动项 `~/.config/autostart/org.fcitx.Fcitx5.desktop`（若有），然后重新登录。
- 还要在 X11 应用里使用 Fcitx5：在 Fcitx5 配置工具的「附加组件」里关闭「Wayland 输入法前端」，然后重启 Fcitx5。

### 安装

先安装 Rust 1.96 或更新版本，以及编译所需的软件包。Debian / Ubuntu 上是：

```sh
sudo apt install pkg-config python3 libssl-dev libxkbcommon0 \
  fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji
```

Arch Linux 上是：

```sh
sudo pacman -S --needed base-devel python openssl libxkbcommon noto-fonts noto-fonts-cjk noto-fonts-emoji
```

中文字体不能省：候选窗口由青简绘制，缺了中文字体候选显示不出来。缺了哪一项，安装脚本开头会列出来。在源码目录执行：

```sh
tools/release/data-fetch.sh
apps/linux/scripts/install.sh --frontend wayland --enable-service
```

- `--enable-service` 让青简服务随登录自动启动，并立即启动 Wayland 原生版。不加时执行 `systemctl --user start qingjian-wayland.service` 启动。
- 若有时也登录 KDE Plasma、GNOME 等桌面、两个版本都需要，则写 `--frontend auto,wayland`。
- 只试安装流程时，可再加 `--sample --debug`，与 fcitx 版相同。

### 随桌面启动

- **niri**（在登录界面选 niri 登录时）以及用 uwsm 启动的桌面：安装时加了 `--enable-service` 即随桌面启动；没加时执行一次：

  ```sh
  systemctl --user enable --now qingjian-wayland.service
  ```

- **Hyprland**：在 `~/.config/hypr/hyprland.conf` 中加一行：

  ```text
  exec-once = systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service
  ```

- **sway**：在 `~/.config/sway/config` 中加一行：

  ```text
  exec systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service
  ```

其他桌面在各自的自启动配置里执行同一条命令。改完重新登录生效；在桌面的终端里执行这条命令（`exec-once =` 或 `exec` 后面的部分）可立即启动。

### 应用里的设置

- GTK 应用、Qt 6.7 及以后的应用、Firefox，以及 foot、kitty、Alacritty、WezTerm 等终端直接可用。
- 环境变量 `GTK_IM_MODULE`、`QT_IM_MODULE` 不要设为 `fcitx` 或 `ibus`（不设，或设为 `wayland`），否则应用绕过青简。它们常写在桌面配置（如 Hyprland 的 `env = QT_IM_MODULE,fcitx`）、`~/.config/environment.d/` 或 `/etc/environment` 中，删掉后重新登录。
- Chromium、Google Chrome 与 Electron 应用（如 VS Code）：启动参数加 `--ozone-platform=wayland --enable-wayland-ime`；在 sway、niri、labwc 上再加 `--wayland-text-input-version=3`。
- 经 XWayland 运行的应用没有输入法。

### 与 fcitx 版的不同

- 候选窗口的主题、字体与字号在「设置 → 外观」里改，见 [外观与主题](../personalize/appearance.md)；外观选「跟随系统」时暂按浅色显示。
- 中英模式所有窗口共用一份；切换时没有提示，也没有托盘图标。
- 英文模式暂时认不出当前应用：取消「设置 → 中英文」的「只在写字的应用里给」后才给英文候选。
- 打字中切换到别的窗口时，没上屏的拼音会丢掉（fcitx 版会原样上屏）。
- 按住 `Shift` 用鼠标选中文字后松开 `Shift`，会切换中英。
- 暂不支持：鼠标点选候选、候选右键菜单、翻译选中的文字，以及上文 fcitx 版暂不支持的各项。
- 执行 `systemctl --user status qingjian-wayland` 可查看运行状态与出错原因；停止工作时执行 `systemctl --user restart qingjian-wayland` 重新启动。

## 更新与卸载

更新：关掉运行青简的终端，重新执行对应的安装命令（预编译包下载新包后再执行 `./install.sh`），然后重启 Fcitx 并重新启动青简。
Wayland 原生版更新后执行 `systemctl --user restart qingjian-server qingjian-wayland` 换上新版本。
安装时会检查文件；你改过的安装文件会提示保留，请先备份。

卸载：

```sh
apps/linux/scripts/uninstall.sh
# 自定义过安装位置时：
apps/linux/scripts/uninstall.sh --prefix /安装时的绝对路径
```

预编译包用包里的 `./uninstall.sh`。卸载会停掉随登录启动的青简服务；手动启动的要自己关掉那个终端。之后重启 Fcitx，用 IBus 的注销后重新登录一次。卸载保留设置、个人词库、学习数据与其他输入法。
Wayland 原生版随卸载一起停掉；之后删掉桌面配置里为它加的那一行，要换回 Fcitx5 的把它的启动项加回去。
系统里另外装过青简（比如旧版的 deb 包）时，重新登录后以源码安装的这份为准；想换回那一份，运行卸载脚本后再重新登录。
