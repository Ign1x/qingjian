# apps/linux/wayland

青简的 Wayland 原生版前端：自己当 `zwp_input_method_v2` 客户端，候选窗用青简渲染器自绘，引擎在 `qingjian-linux-server` 进程里。
为什么这么做、覆盖哪些合成器与应用，见 [docs/design/linux-wayland.md](../../../docs/design/linux-wayland.md)；实现要点见 [crate-notes](../../../docs/notes/crate-notes.md) 的 apps/linux/wayland 一节。

现在是原型：只验过 Hyprland 与嵌套 sway。装到用户目录见下面「安装」，开发时按「构建与运行」手动跑。

## 前提

- 合成器有 input-method-v2 与虚拟键盘：Hyprland、sway、niri、labwc 等。KWin、GNOME 没有，用 fcitx 版。
- 按应用记中 / 英、`[apps]` 按应用的设置要合成器另有 `zwlr_foreign_toplevel_manager_v1`（上面几个都有）；没有时所有应用共用一个会话，启动日志里有一句。
- **同一个 seat 只能有一个输入法**：fcitx5 的 `waylandim` 插件开着时这里会收到 `unavailable` 退出。试之前先停掉 fcitx5（或在它的配置里关掉 waylandim）。
- Server 起着：`cargo run -p qingjian-linux-server`（或装好的 systemd 用户服务），两边用同一个 socket（`QINGJIAN_SOCKET` 可改）。
- 应用走 Wayland 的 text-input：`GTK_IM_MODULE` / `QT_IM_MODULE` 不要设成 `fcitx` / `ibus`（Qt 6.7 起设 `wayland` 或不设）；
  Chromium / Electron 加 `--enable-wayland-ime`，只认 text-input-v3 的合成器上再加 `--wayland-text-input-version=3`。XWayland 应用没有输入法。

## 安装

与 fcitx 版同一个脚本，装到 `~/.local`（用户怎么装、各合成器的自启写法见用户文档 [Linux](../../../docs/user/platforms/linux.md) 的「Wayland 原生版」一节）：

```bash
tools/release/data-fetch.sh
apps/linux/scripts/install.sh --frontend wayland --enable-service    # 两个版本都要：--frontend auto,wayland
```

- 装上 Server、设置程序、这个前端，以及两个 systemd 用户单元：`qingjian-server.service` 与 `qingjian-wayland.service`
  （模板 `apps/linux/scripts/qingjian-wayland.service.in`：Wants / After Server；PartOf / After / WantedBy `graphical-session.target`；
  会话没把 `WAYLAND_DISPLAY` 导入 systemd 时不起）。`--enable-service` 把两个都 `enable --now`。
- **重启策略**：`Restart=on-failure` 但 `RestartPreventExitStatus=2`。前端起不来的原因（合成器缺协议、seat 被占、没有 libxkbcommon、连不上合成器）都退出码 2，
  重启也没用，原因在 `journalctl --user -u qingjian-wayland`；跑着跑着与合成器的连接断了、协议出错是 1，与 panic（101）、被信号杀掉一样照常重启。
- **随桌面启动**：niri（`niri-session`）、uwsm 这类会启动 `graphical-session.target` 的会话里 enable 就够；Hyprland、sway 直接启动时没有这个 target，
  在配置里加 `exec-once = systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service`（sway 写 `exec`）。
  先导入再起：合成器自己也会异步导入环境，赛跑输了单元的 `ConditionEnvironment` 不满足、不起。
- 前端单独一次 `cargo build -p qingjian-linux-wayland`（见 crate-notes），装好的二进制只链 libc / libm / libgcc_s，libxkbcommon 运行时 dlopen。
- 发行包是单独的 `qingjian-wayland`，不随包 enable，见 [linux-packaging.md](../../../docs/notes/linux-packaging.md)。
- 卸载 `apps/linux/scripts/uninstall.sh`：先停这个前端、再停 Server，合成器配置里加的那一行要自己删。

**NixOS**：libxkbcommon 不在缺省的库搜索路径里。install.sh 发现它只能从 `LD_LIBRARY_PATH` 找到时，把那个目录写进单元的 `Environment=`；
store 路径会被垃圾回收，先给它建一个 GC root 再装：

```bash
nix-build '<nixpkgs>' -A libxkbcommon -o ~/.local/lib/qingjian-xkbcommon
nix-shell -p pkg-config openssl --run \
  "LD_LIBRARY_PATH=$HOME/.local/lib/qingjian-xkbcommon/lib apps/linux/scripts/install.sh --frontend wayland --enable-service"
```

字体检查与渲染器一样，固定路径都不在时问 fontconfig，NixOS 上不用另装。

## 构建与运行

```bash
cargo build -p qingjian-linux-wayland      # 单独编：和设置程序一起编会被统一成 dlopen libwayland 的版本
RUST_LOG=debug target/debug/qingjian-linux-wayland --stderr
```

libxkbcommon 是运行时 dlopen 的。NixOS 上它不在默认的库搜索路径里：

```bash
LD_LIBRARY_PATH=$(nix-build '<nixpkgs>' -A libxkbcommon --no-out-link)/lib target/debug/qingjian-linux-wayland --stderr
```

不加 `--stderr` 时日志写 `$XDG_STATE_HOME/qingjian/logs/wayland.<日期>.log`，与 Server 的日志在同一个目录。
排查协议往来时加 `WAYLAND_DEBUG=1`（会打印按键与上屏内容，只在自己机器上用）。
鼠标点不中时看 `RUST_LOG=debug` 的「候选窗上能点的地方」（各候选与菜单项中心的 surface 坐标）与「鼠标按在候选窗上」，对照 `WAYLAND_DEBUG=1` 里合成器给的指针坐标。

装好之后的开机启动见上面「安装」；手动跑的开发版不要与装好的服务同时开着（同一个 seat 只能有一个输入法，后起的那个收到 `unavailable`）。

## 测试

```bash
cargo test -p qingjian-linux-wayland
```

会话规则（按键转发、抬起跟着按下走、过期回包、上屏切块、重复、Server 断开重连、按应用分会话、选区翻译、鼠标点选与右键菜单）、input-method-v2 的状态机、活动窗口的跟踪、
位图裁边与缩放、鼠标落点与菜单合成、滚轮折成翻页都是纯逻辑，不要合成器；
与 Server 的连接（一个应用一个会话、关最久没用的、重连补开、选区与鼠标请求）对着走真 socket 的假 Server 测。

`tests/blackbox/` 是黑盒测试：真的二进制连测试进程里的假合成器（wayland-server）与假 Server，过一遍起不来的两种退出、SIGTERM、
接管键盘、行内拼音与上屏、放行的键经虚拟键盘转出、失活时还按着的键、候选窗贴图与分数缩放、密码框与敏感框、前后文、组句中应用改了文字、选区翻译的一整轮，
以及鼠标（点候选上屏、滚轮翻页、右键菜单与点项、Esc 收菜单、旧落点竞态与缓冲背压）；
每个测试结束时查被测进程以 0 退出、交还了键盘、没留下按着的键。鼠标测试逐种允许对应请求，假 Server 收到未允许或不合规矩的消息仍算失败。怎么搭的见 crate-notes。
二进制运行时 dlopen libxkbcommon，加载不了时这些测试打印「跳过 …」后通过（CI 里 ubuntu:22.04 那个裸容器就是这样）。
设了 `QINGJIAN_BLACKBOX_REQUIRED=1` 时改成失败，不许跳过：CI 的 Linux job 装好 libxkbcommon 与 libwayland-client 后就这样跑；本地不设，行为不变。NixOS 上：

```bash
LD_LIBRARY_PATH=$(nix-build '<nixpkgs>' -A libxkbcommon --no-out-link)/lib cargo test -p qingjian-linux-wayland
```

与整个 workspace 一起跑（`cargo test --workspace`）时前端改成 dlopen libwayland-client，`LD_LIBRARY_PATH` 里还要加上
`$(nix-build '<nixpkgs>' -A wayland --no-out-link)/lib`，不然同样跳过。
真会话里要过的一遍见设计文档的「验证」一节。

## 验证记录

- 2026-10-28，嵌套 sway 1.12（headless）+ foot 1.28：逐项过了一遍，清单见设计文档的「验证」一节。
- 2026-10-28，Hyprland（HyDE，NixOS，DP-2 1.5 倍）：开发版 Server（私有 socket、空白学习数据、0.2.1 的产品数据）+ release 版前端，
  停掉 fcitx5 后日常输入可用，全拼与小鹤双拼都试过，用户反馈体验好。各应用逐个的结果（kitty、GTK、浏览器、密码框）还没逐条记。
