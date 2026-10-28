# Linux 发行包：deb / rpm 与预编译 tarball（2026-09-23）

`apps/linux/packaging/build.sh` 一条命令打 deb / rpm，`apps/linux/scripts/package.sh` 打用户级的预编译 tarball（Fcitx5 版与 Wayland 版），都给别的 Linux 机器装着试。
这一页记这几种包各装了什么、为什么这么分、怎么装，以及几处绕不过去的限制。
flatpak 暂不做（2026-10-23）：输入法在沙箱里要靠宿主机上的登记脚本去写服务与 IBus 组件，等于绕开沙箱，出了问题也难查。
用户级的源码安装（`apps/linux/scripts/install.sh`）另见 [linux-fcitx5.md](linux-fcitx5.md) 与 [linux-dual-frontend.md](linux-dual-frontend.md)，
Wayland 原生版的见 [apps/linux/wayland/README.md](../../apps/linux/wayland/README.md)。

## 打包

```bash
apps/linux/packaging/build.sh               # 两种都打
apps/linux/packaging/build.sh deb rpm       # 只打给出的几种
QINGJIAN_SAMPLE=1 apps/linux/packaging/build.sh   # 没有产品数据时只带样例词库
```

产物在 `target/linux-pkg/out/`。只要 docker（免 sudo），本机不用装 fcitx5 开发包或 rpmbuild：

| 步骤 | 在哪做 | 为什么 |
|---|---|---|
| Rust 的 Server、IBus 前端与 Wayland 前端 | Ubuntu 22.04 容器，编一次两种包共用 | 这三个只依赖 libc / libm / libgcc；在 glibc 2.35 上编，实际门槛是 `GLIBC_2.34`。工具链与 crate 缓存直接挂本机的 `~/.cargo` / `~/.rustup`；依赖树里有 `openssl-sys`，编译要 `libssl-dev`（最终二进制并不链 libssl）。Wayland 前端单独一次 `cargo build`：与别的包同编时 cargo 合并依赖的特性，带进 winit 的包会把 wayland-backend 换成 dlopen libwayland 的那种（crate-notes 的 apps/linux/wayland 一节） |
| fcitx5 插件（deb） | Debian 13 容器 | 插件要对着目标发行版自己的 fcitx5 编；`CMakeLists.txt` 要 5.1.8+，Ubuntu 24.04 只有 5.1.7 |
| fcitx5 插件（rpm） | Fedora 42 容器 | 同上 |
| 随包资源 | 本机，`files.py resources` | 与 `install.sh` 同一套产品数据校验（`tools/release/data.lock`）与布局 |

## 两种包各装了什么

deb 与 rpm 都拆成三个包：

- **`qingjian`**：`/usr/bin/qingjian-linux-server`、`/usr/bin/qingjian-linux-ibus`、`/usr/share/qingjian/resources/`（词库等，
  Server 按「可执行文件上一层的 `share/qingjian/resources`」找到它）、`/usr/share/ibus/component/qingjian.xml`、
  `/usr/lib/systemd/user/qingjian-server.service`。装的时候 `systemctl --global enable` 让每个用户登录后自动起 Server。
- **`qingjian-fcitx5`**：fcitx5 插件与两份描述文件，放 fcitx5 自己的插件目录（Debian 是 `/usr/lib/<多架构>/fcitx5/`，Fedora 是 `/usr/lib64/fcitx5/`）。
- **`qingjian-wayland`**（2026-10-28 起）：Wayland 原生版的前端 `/usr/bin/qingjian-linux-wayland` 与
  `/usr/lib/systemd/user/qingjian-wayland.service`（与 `install.sh` 同一个模板，见 [design/linux-wayland.md](../design/linux-wayland.md)）。
  依赖 `qingjian` 同版本；libxkbcommon 是前端运行时 dlopen 的，自动推出的依赖里没有，手写：deb `libxkbcommon0`，rpm 写 soname `libxkbcommon.so.0()(64bit)`
  （Fedora 的 `libxkbcommon` 与 openSUSE 的 `libxkbcommon0` 都提供它）。字体（Noto Sans CJK、Noto Color Emoji、fontconfig）只放 Recommends：
  缺了候选窗画不好中文与表情，但前端照样能起。

IBus 前端没有额外依赖，跟 Server 同包；它的组件 XML 在系统目录，IBus 直接认，不用 `install.sh` 那套 environment.d。

### Wayland 前端为什么单独一个包，而且不随包启用

- **两个版本是二选一的**：同一个 seat 只能有一个输入法，fcitx5 的 waylandim 开着时 Wayland 前端收到 `unavailable` 退出；
  KDE、GNOME 根本没有它要的协议。跟 Server 同包的话，装 IBus / fcitx5 的人也拿到一个用不上、还得解释的服务；拆开后装哪个包就是选哪个版本。
- **依赖不同**：只有它要 libxkbcommon 与自绘候选窗的字体，不该压到只用 IBus 的人头上。
- **不 `--global enable`**：Server 的单元对所有用户全局 enable，Wayland 前端不能这么做——在 GNOME / KDE 会话、或还开着 fcitx5 的会话里，
  每次登录都会起一个必然失败的前端。装包只把单元放进 `/usr/lib/systemd/user/`，要用的用户自己 `systemctl --user enable --now qingjian-wayland.service`，
  第一次装时 postinst / `%post` 打印这句与「关掉 fcitx5」。单元 `WantedBy=graphical-session.target`：niri（niri-session）、uwsm 这类会启动它的会话里 enable 就随登录起来，
  Hyprland / sway 直接启动的要在配置里 `systemctl --user start`（用户文档 [Linux](../user/platforms/linux.md) 的 Wayland 原生版一节有各合成器的写法）。
- **升级与卸载**：postinst / `%post` 对每个已登录用户 `daemon-reload` 后 `try-restart`（只重启正开着的，换上新的二进制）；
  prerm / `%preun` 卸载时 `--global disable`（管理员手动全局 enable 过时清掉）并 stop 每个已登录用户的实例。
  用户自己 enable 留在 `~/.config/systemd/user/graphical-session.target.wants/` 的链接包管理器不碰，卸载后是一个悬空链接，systemd 忽略它。

## 预编译 tarball（package.sh）

`apps/linux/scripts/package.sh` 在本机打用户级包，不用 root：解开、跑一句 `install.sh` 就装进 `~/.local`。
缺省打 Fcitx5 版，`--frontend wayland` 打 Wayland 原生版：

```bash
tools/release/data-fetch.sh                       # 产品数据与含章模型，两版都要
apps/linux/scripts/package.sh                     # target/linux-package/qingjian-<版本>-linux-<cpu>.tar.gz
apps/linux/scripts/package.sh --frontend wayland  # target/linux-package/qingjian-wayland-<版本>-linux-<cpu>.tar.gz
```

两版都在本机编，不经过容器，文件按仓库相对路径放进包根：

- **Fcitx5 版**：`bin/qingjian-linux-server`、`bin/qingjian-settings`、`lib/qingjian.so`（fcitx5 插件，cmake 编）与它的描述文件 `apps/linux/fcitx5/data`。
- **Wayland 版**：`bin/` 下是 `qingjian-linux-server`、`qingjian-settings`、`qingjian-linux-wayland`；**不编 fcitx5 插件、不跑 cmake**，
  改在包根放两个单元模板 `qingjian-server.service.in` 与 `qingjian-wayland.service.in`，装的时候按安装路径填好 `@EXEC@`。
- 两版都带 `install.sh`、`uninstall.sh`、`files.py`、`LICENSE`、`apps/settings/linux`、`assets/icon/linux/hicolor`、
  `assets/stroke/LICENSE-CNS11643.txt`、`tools/release/data.lock`、`assets/sample` 等产品数据目录，以及 `target/release-data/qingjian-data.tar.gz`
  （压缩包带的是大块词库与模型，装前解开，`files.py` 先按 `data.lock` 校验压缩包、再按包内摘要逐个校验解出的文件）。

Wayland 前端在 Server 与设置程序那次 `cargo build` 之外再单独编一次：同一次编时 cargo 会合并依赖的特性，
别的包带进来的 winit 会把 wayland-backend 换成 dlopen libwayland 的那种，单独编的只 dlopen libxkbcommon（见 [crate-notes](crate-notes.md) 的 apps/linux/wayland 一节）。

### 装

解开后 `./install.sh`，`--prefix` 改安装位置（缺省 `~/.local`）。包里的 `install.sh` 是 `package.sh` 从 `package-install.sh` 拷出来的，
拷时已把版本那行（`frontend=`）改成这个包的前端，**装的人不用再选**。装 Server、设置程序与对应的前端，文件清单记在
`$prefix/share/qingjian/install-manifest.json`，`uninstall.sh` 按清单删（用户数据保留）。

依赖：两版都要 python3；Fcitx5 版查 `fcitx5`，Wayland 版**不查 fcitx5**，查系统 UI 字体、中文字体与 libxkbcommon。
libxkbcommon 只能从 `LD_LIBRARY_PATH` 找到时（NixOS）把那个目录写进 Wayland 单元的 `Environment=`。

装完不自动 enable。Wayland 版打印这一句，要用的用户自己跑：

```bash
systemctl --user daemon-reload && systemctl --user enable --now qingjian-server.service qingjian-wayland.service
```

同一时间只能有一个输入法，enable 之前先关掉 fcitx5（或它的 Wayland 输入法前端）；Hyprland / sway 这类直接启动的桌面没有
`graphical-session.target`，要把 `exec-once = systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service`
写进桌面配置。

**发版还没接**：`.github/workflows/release.yml` 只跑缺省版，smoke 测试与上传也按单份包写；`tools/release/releases_json.py` 的附件名规则
`^qingjian-.+-linux-x86_64\.tar\.gz$` 会把 `qingjian-wayland-…-linux-x86_64.tar.gz` 也认成 linux/x86_64，与 Fcitx5 版在 releases.json 里撞同一个键。
所以 Wayland tarball 现在只在本机打，下载页上没有它。

## 装法

deb / rpm 装完 Server 就在已登录用户的会话里起来了（装包脚本对正在跑的用户 systemd 实例直接 restart）；
IBus 要重新登录一次（或 `ibus restart`）才看得到新引擎。

### Debian / Ubuntu（deb）

```bash
sudo apt install ./qingjian_<版本>_amd64.deb                 # IBus 用户
sudo apt install ./qingjian_<版本>_amd64.deb ./qingjian-fcitx5_<版本>_amd64.deb   # fcitx5 用户
sudo apt install ./qingjian_<版本>_amd64.deb ./qingjian-wayland_<版本>_amd64.deb  # Wayland 原生版（Hyprland、sway、niri…）
```

`qingjian` 只要 `libc6 (>= 2.34)`，Debian 12、Ubuntu 22.04 / 24.04 也装得上（IBus）；
`qingjian-fcitx5` 要 `libfcitx5core7 (>= 5.1.12)`，只有 Debian 13、Ubuntu 25.04 及以后；
`qingjian-wayland` 与 `qingjian` 一样只看 glibc（另加 `libxkbcommon0`），能不能用看合成器：sway 要 1.10 起才有候选窗。

### Fedora 42 及以后（rpm）

```bash
sudo dnf install ./qingjian-<版本>.x86_64.rpm
sudo dnf install ./qingjian-<版本>.x86_64.rpm ./qingjian-fcitx5-<版本>.x86_64.rpm
sudo dnf install ./qingjian-<版本>.x86_64.rpm ./qingjian-wayland-<版本>.x86_64.rpm
```

KDE Plasma（Wayland）上输入法由 KWin 拉起，默认是 fcitx5：「系统设置 → 键盘 → 虚拟键盘」里选 IBus Wayland
（等于 `kwriteconfig6 --file kwinrc --group Wayland --key InputMethod /usr/share/applications/org.freedesktop.IBus.Panel.Wayland.Gtk3.desktop`），
装了 fcitx5 的再把它的自启关掉，重新登录。

### 装完之后

- IBus：「设置 → 键盘 → 输入源」添加「青简」（GNOME），或 `ibus engine qingjian`。
- fcitx5：`fcitx5-configtool` 里取消「仅显示当前语言」，添加「青简」。
- Wayland 原生版：关掉 fcitx5（或它的「Wayland 输入法前端」附加组件），`systemctl --user enable --now qingjian-wayland.service`；
  Hyprland / sway 另在配置里起它，见用户文档。起不来的原因：`journalctl --user -u qingjian-wayland`（退出码 2 是这个桌面跑不了，不重启；1 是运行中出错，会重启；见单元模板的注释）。
- 配置：`~/.config/qingjian/config.toml`，保存后约一秒内生效（`preedit` 与 `input_log` 两项要重启 Server）。
- 自检（不经过输入法框架，直接问 Server）：`qingjian-linux-ibus --check nihao`。
- Server 状态与日志：`systemctl --user status qingjian-server`、`journalctl --user -u qingjian-server`。

## 实测（2026-09-23，容器里装包 + 起 Server + `--check nihao`）

| 包 | 装在 | 结果 |
|---|---|---|
| deb 两个包 | Debian 13、Ubuntu 26.04 | 装上；插件不缺库；`systemctl --global` 自启登记上、卸载后清掉；自检出「你好」等候选 |
| deb `qingjian` | Ubuntu 24.04、22.04 | 同上（不装插件） |
| rpm 两个包 | Fedora 42、Fedora 最新、openSUSE Tumbleweed | 同上 |

容器里没有图形会话，**真会话里切到青简打字没验**：这一步要在别的机器上装了之后手测，判据见 [linux-dual-frontend.md](linux-dual-frontend.md) 第二节。

### 装包自检：`smoke/run.sh`

```bash
apps/linux/packaging/build.sh deb rpm
apps/linux/packaging/smoke/run.sh                 # 五个发行版都跑；给镜像名只跑那几个
```

把 `target/linux-pkg/out/` 的 deb / rpm 拿到 Debian 13、Ubuntu 26.04、Fedora 42、最新 Fedora、openSUSE Tumbleweed 的干净容器里：
装三个包，看 `systemctl --global` 自启登记、IBus 组件的 `<exec>` 指向的文件在不在、fcitx5 插件缺不缺库，
Wayland 前端能执行（`--version`；容器里没有合成器，跑不了真的）、手写的 libxkbcommon 依赖真把 `libxkbcommon.so.0` 装上了、它的单元在而没有被全局 enable，
起 Server 用包里的 `qingjian-linux-ibus --check nihao` 打一串键（走真前端那套客户端代码，不另抄协议），
等本地整句模型加载完，卸载后看自启登记与组件清掉没有；哪一项不对退出码非 0。每个容器要装发行版的依赖，跑一遍五六分钟。
2026-10-14 五个都过（data-v7）：候选「你好 👋 (＾▽＾)ノ 你好好 你好像…」，含章·通变加载并预热。
2026-10-28 加了 `qingjian-wayland` 之后还没在容器里跑过：打包与自检脚本只过了 `bash -n` 与 shellcheck，下次打包时先跑一遍 smoke。

### fcitx5 的依赖与切换框架

`apt install ./…` / `dnf install ./…` 会把 fcitx5 与它的 GTK / Qt 输入法模块、配置工具一起装上（2026-09-23 在干净的
Debian 13、Ubuntu 26.04、Fedora 42 容器里看过；`dpkg -i` 不补依赖，要再跑一次 `sudo apt -f install`）。
**不会替你把输入法框架从 IBus 切到 fcitx5**，GNOME 默认是 IBus：

- Debian / Ubuntu：`im-config -n fcitx5`，重新登录；
- Fedora：装 `fcitx5-autostart`（`qingjian-fcitx5` 已经 Recommends 它，dnf 缺省会一起装），重新登录。

## 几处限制与坑

- **`systemctl --global enable` 不够**：它只对之后起来的用户 systemd 实例生效，而注销再登录得快时实例根本不重起。
  2026-09-23 在 Ubuntu 24.04 上踩过：10:51 装包、10:52 重新登录，用户实例是 10:46 起的一直没换，Server 一次都没起，
  切到青简打字没有候选。所以 postinst / `%post` 对 `loginctl list-users` 里的每个用户 `systemctl --user -M 用户@ restart`，
  prerm / `%preun` 同样 stop。
- **与用户级安装冲突**：`install.sh` 装在 `~/.local` 的那一份也叫 `qingjian-server.service`，组件名也相同（Wayland 前端的 `qingjian-wayland.service` 同理）。
  用户目录的单元优先于系统目录的，两份同时在时生效的是用户目录那份；装发行包之前先跑 `apps/linux/scripts/uninstall.sh`。
  2026-09-23 本机踩过：装了 deb 没卸 `~/.local` 那份，IBus 拉起的是 deb 的新前端（协议 7），跑着的却是用户目录单元起的旧 Server
  （协议 6），开会话就被断开、按键全部放行，切到青简打不出字；前端的输出进了 `/dev/null`，`--check` 按 PATH 又先找到 `~/.local/bin`
  的旧前端、看上去是通的。现在两处会提醒：装包时 postinst / `%post` 发现有用户的 `~/.config/systemd/user/qingjian-server.service`
  就打印卸法；Server 碰到版本不一致记 `前端开会话带的协议版本与 Server 不一致` 的警告（`journalctl --user -u qingjian-server`）。
  确认跑的是哪一份：`systemctl --user show qingjian-server -p FragmentPath`。
- **IBus 的注册表缓存不跟 `IBUS_COMPONENT_PATH` 走**：ibus-daemon 把组件缓存在 `~/.cache/ibus/bus/registry`，
  下次启动只看缓存里记着的那几个目录变没变，路径变量换了它不管。2026-09-23 在 Arch（KDE Plasma）上踩过：先登记了一份青简、
  后 `pacman -S ibus`，登记时 `/usr/share/ibus/component` 还不存在，脚本退到了 `/usr/local/...`；会话里的 daemon 按这个路径写了缓存，
  里面没有系统组件目录，之后每次启动都找不到 dconf 配置组件，报 `Can not execute default config program` 退出，
  界面上是「IBus 守护进程 5 秒内无法启动」，重新登录、改对变量都没用，删掉缓存才好。现在 `install.sh` 只在
  `/usr/local` 下确实有 IBus 时才换目录，装与卸都删掉这份缓存。
- **挑 IBus 地址不能只看 socket 文件在不在**：ibus-daemon 退出不删 socket，`~/.cache/ibus` 下常年一堆死的；
  陈旧的 `…-unix-0` 按字典序排在 `…-unix-wayland-0` 前面，指着一个死 socket，前端连它失败，切到青简 `SetGlobalEngine` 超时。
  现在真 connect 一下，连得上才认。
- **`--global` 对登录界面也生效**：gdm 这类系统账户也有自己的用户会话（`loginctl list-users` 里能看到 `gdm`），
  不拦的话 greeter 里也会起一个 Server、白白加载词库与模型。服务单元写 `ConditionUser=!@system` 跳过系统账户（systemd 237+）。
  装包脚本取用户名用 shell 的 `read`，不靠 awk（openSUSE 最小容器里没有 awk）。
