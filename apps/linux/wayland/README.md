# apps/linux/wayland

青简的 Wayland 原生版前端：自己当 `zwp_input_method_v2` 客户端，候选窗用青简渲染器自绘，引擎在 `qingjian-linux-server` 进程里。
为什么这么做、覆盖哪些合成器与应用，见 [docs/design/linux-wayland.md](../../../docs/design/linux-wayland.md)；实现要点见 [crate-notes](../../../docs/notes/crate-notes.md) 的 apps/linux/wayland 一节。

现在是原型：只验过 Hyprland，还没有安装脚本与 systemd 服务，按下面手动跑。

## 前提

- 合成器有 input-method-v2 与虚拟键盘：Hyprland、sway、niri、labwc 等。KWin、GNOME 没有，用 fcitx 版。
- 按应用记中 / 英、`[apps]` 按应用的设置要合成器另有 `zwlr_foreign_toplevel_manager_v1`（上面几个都有）；没有时所有应用共用一个会话，启动日志里有一句。
- **同一个 seat 只能有一个输入法**：fcitx5 的 `waylandim` 插件开着时这里会收到 `unavailable` 退出。试之前先停掉 fcitx5（或在它的配置里关掉 waylandim）。
- Server 起着：`cargo run -p qingjian-linux-server`（或装好的 systemd 用户服务），两边用同一个 socket（`QINGJIAN_SOCKET` 可改）。
- 应用走 Wayland 的 text-input：`GTK_IM_MODULE` / `QT_IM_MODULE` 不要设成 `fcitx` / `ibus`（Qt 6.7 起设 `wayland` 或不设）；
  Chromium / Electron 加 `--enable-wayland-ime`，只认 text-input-v3 的合成器上再加 `--wayland-text-input-version=3`。XWayland 应用没有输入法。

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

Hyprland 开机启动：在配置里加 `exec-once = qingjian-linux-wayland`，并把 fcitx5 的启动项去掉。

## 测试

```bash
cargo test -p qingjian-linux-wayland
```

会话规则（按键转发、抬起跟着按下走、过期回包、上屏切块、重复、Server 断开重连、按应用分会话）、input-method-v2 的状态机、活动窗口的跟踪、位图裁边与缩放都是纯逻辑，不要合成器；
与 Server 的连接（一个应用一个会话、关最久没用的、重连补开）对着走真 socket 的假 Server 测。
真会话里要过的一遍见设计文档的「验证」一节。

## 验证记录

- 2026-10-28，嵌套 sway 1.12（headless）+ foot 1.28：逐项过了一遍，清单见设计文档的「验证」一节。
- 2026-10-28，Hyprland（HyDE，NixOS，DP-2 1.5 倍）：开发版 Server（私有 socket、空白学习数据、0.2.1 的产品数据）+ release 版前端，
  停掉 fcitx5 后日常输入可用，全拼与小鹤双拼都试过，用户反馈体验好。各应用逐个的结果（kitty、GTK、浏览器、密码框）还没逐条记。
