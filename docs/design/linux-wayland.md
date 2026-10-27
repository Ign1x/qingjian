# Linux Wayland 原生版

2026-10-27 定。Linux 分成两个版本：**fcitx 版**（现有：Fcitx5 / Fcitx4 / IBus 前端，候选窗由框架画）与 **Wayland 原生版**（本文）：
青简自己当 `zwp_input_method_v2` 客户端，候选窗用 `qingjian-render` 自绘，和 macOS / Windows 同一套主题与显示面。
代码在 `apps/linux/wayland`，开发怎么跑见那里的 README。

## 为什么分两个版本

fcitx 版的显示面全归框架：主题、译词的多条义项与生词色、拼音行、动画都用不上，分数缩放下面板糊或错位还得靠框架设置去凑。
[candidate-ui.md](candidate-ui.md) 的平台表早就把 Linux 定成「自绘窗口」，难点在 Wayland 上普通客户端开不了跟着光标的窗口——
能开的只有输入法客户端：input-method-v2 的 `zwp_input_popup_surface_v2` 由合成器贴在光标旁。所以要自绘，青简就得自己当输入法，不能再挂在框架下面。

代价是覆盖面变窄，两个版本互补，不互相替代：

| 合成器 | input-method-v2 | 用哪个版本 |
|---|---|---|
| Hyprland、sway（1.10 起有 popup）、niri、labwc、mango 等 wlroots / smithay 系 | 有，含 popup 与 virtual-keyboard | Wayland 原生版 |
| KWin（Plasma 6.8） | 只有 v1，且只给 KWin 自己拉起的那一个输入法 | fcitx 版 |
| GNOME / Mutter | 没有 | fcitx 版（IBus） |

应用侧只覆盖 Wayland 原生应用：GTK3 / 4、Qt6（6.7 起支持 text-input-v3，`QT_IM_MODULE` 不设或设 `wayland`）、Firefox、foot / kitty / alacritty / wezterm、
Chromium / Electron（加 `--enable-wayland-ime`；只认 text-input-v3 的合成器上再加 `--wayland-text-input-version=3`）。
**XWayland 应用没有输入法**（WPS 这类）——要它们就用 fcitx 版。`GTK_IM_MODULE` / `QT_IM_MODULE` 不能设成 `fcitx` / `ibus`，否则应用不走 text-input。

同一个 seat 只能有一个 input-method-v2 客户端：fcitx5 的 `waylandim` 插件开着时，青简收到 `unavailable`，日志写明原因后退出。

## 进程与数据流

```
合成器 ──activate / done / surrounding_text / content_type；grab 的 keymap / key / modifiers / repeat_info──▶ qingjian-linux-wayland
       ◀──commit_string / set_preedit_string / commit(serial)；popup 缓冲；virtual_keyboard 转发放行的键──┘
qingjian-linux-wayland ──Unix socket，protocol::linux（与 Fcitx5 插件、IBus 前端同一套）──▶ qingjian-linux-server
```

- **Server 不改。** Wayland 前端是 Server 的又一个客户端：吃不吃一个键、上什么屏、帧里有什么、整句重排与云端候选，全由 Server 定（[contributing.md](../contributing.md)「架构约束」），
  前端只把合成器的事件翻成 `LinuxEvent`，把回包翻成 input-method-v2 的请求。fcitx 版与 Wayland 版用同一个 Server 二进制。
- **渲染器在前端进程。** popup surface 只能由持有 `zwp_input_method_v2` 的客户端创建，所以在前端里画、前端里贴。
  [architecture.md](architecture.md) 把 Linux 的渲染器放 Server，理由是不进 fcitx5 那个所有输入法共用的进程；对青简自己的前端进程不成立。
  词库、语言模型、神经模型仍只在 Server。
- **Server 不推送。** 组句中或 `translation_pending` 时每 80 ms `Poll`，回包 `identity` 的 generation / context 对得上、revision 变大才重画，与两个现有前端一样。
- 客户端（连接、回包、键表、前后文截取）在 `qingjian_platform::protocol::linux`，IBus 前端与 Wayland 前端共用；帧换算（协议帧 → 渲染器的帧）在 `qingjian_platform::display`。

## 协议对照

| 合成器给的（`done` 之后生效） | 前端做的 |
|---|---|
| 新的 activate | 报能力（变了才报），然后 `Focus{true}`，成功就 grab 键盘（密码框、Server 连不上时不 grab）；已经激活着又来一次（同一个应用里换了输入框）先按失活处理 |
| deactivate | 挂空缓冲藏起 popup；`Focus{false}` + `Deactivate{focus_out: true, client_preedit: true, capability_changed: false}`：失焦后已经不能往原来的输入框上屏，组句丢弃 |
| content_type | purpose password / pin 或 hint hidden_text → `Capabilities{password}`；hint sensitive_data → `sensitive`。每次 `done` 都重算（密码框点「显示密码」会改它） |
| surrounding_text | 每键随 `Key` 带上光标两侧各 80 字（跳过选区，规则同 Fcitx5 的 `apps/linux/surrounding.h`，单位换成字节） |
| text_change_cause = other | 组句中光标被挪走、文字被应用改了：报 `Reset`，清 preedit |
| grab 的 key | 见下节「按键」 |
| unavailable | 别的输入法占着 seat：日志写明，非零码退出 |

回包落地的顺序：`commit_string` → `set_preedit_string` → `commit(serial)`，serial 是收到过的 `done` 个数。
合成器按「去掉旧 preedit → 删前后文 → 插上屏文字 → 插新 preedit」应用，所以先上屏再换 preedit 的顺序是协议保证的。另有三条：

- **每次 commit 都带完整 preedit**（清空时显式给空串）：text-input-v3 的应用每个 `done` 都会重设 preedit，漏带就闪掉。只在上屏文字或 preedit 变了时才发。
- **等应用落定再发下一次。** foot 收到 serial 对不上的 `done` 会把整次更新（连同上屏文字）丢掉，而它应用完一次更新、重画后才 commit，
  这一帧左右里发出去的下一次正好撞上（嵌套 sway 里连发 `nihao`，行内拼音停在 `ni'ha`）。所以发出一次文字更新后，等合成器转来应用的 `done`（最多等 80 毫秒）再发下一次，
  中间来的合并起来（上屏文字拼在一起，拼音取最新的）；要把键转给应用时先把攒着的发出去，保证先上屏、后按键。
- **单条消息不过 4 KB**：Wayland 一条消息约 4 KB 封顶，超了整条连接断掉、grab 跟着没了。上屏文字按字符边界切成不超过 4000 字节的块，一块一次 commit；preedit 截到 4000 字节。
- **过期回包丢弃。** 等 Server 的那 200 ms 里合成器可能已经发了 deactivate / activate。应用回包前先非阻塞地读一次 Wayland 事件，
  焦点变过就丢掉这次的上屏、preedit 与 popup 更新（放行的键照转）——不能靠 commit serial 兜底，wlroots 与 Hyprland 实测都不按它丢弃。

行内 preedit 放不放按帧里的 `preedit_mode`（`inline` / `window` / `both`），不按握手：握手里那份是 Server 启动时取的，热加载后会过期。

不从合成器来、要前端自己发起的那几样，Server 一侧都已就绪：

| 前端的操作 | 发给 Server 的 | Server 回 |
|---|---|---|
| 每次回包 | — | `KeyResult` / `Update` 上附 `mode`（`english` / `traditional` / `full_width` / `scheme`），托盘、中 / 英提示与主题的 `mode.*` 都照它画；切换的那一下另有 `english` |
| popup 上点候选 / 翻页 | `Candidate{identity, index}` / `Page{identity, next}`，带画这一页的那次回包的身份 | `KeyResult`；帧换过了回 `Ignored` |
| 选区翻译快捷键（`surrounding_text` 里有选区） | `Key{selection_supported: true}` | `RequestSelection{request}` → 前端发 `Selection{request, text}` → `KeyResult`（「翻译中…」，`translation_pending` 时接着轮询） |

客户端（`qingjian_platform::protocol::linux::client::Connection`）对应 `candidate` / `page` / `selection`，`key` / `key_flushing` 多一个 `selection_supported`。

## 按键

- **grab 时机。** activate 生效后 grab；deactivate 之后如果还有键按着，先保持 grab、原样转发到全部抬起再 release，免得补发的抬键落到新焦点上。
- **键表。** grab 给的是 evdev 键码，用 xkbcommon（`xkbcommon-dl`，运行时 dlopen）按 grab 的 keymap 与 `modifiers` 算出 keysym 和 X11 修饰键掩码，
  再走与 Fcitx5 插件一字不差的 keysym → `KeyEvent` 表（`qingjian_platform::protocol::linux::key`）。
  `character` 按 keysym 查 Unicode，不用 xkb 的 `key_get_utf8`：后者对 Ctrl 组合做控制字符变换，Ctrl+A 会变成 0x01。
- **转发。** Server 放行的键经 `zwp_virtual_keyboard_v1` 原样转给应用。虚拟键盘与 IM 在同一条 Wayland 连接上（合成器只对 IM 自己的虚拟键盘不再 grab，不然转出去的键又回到 grab），
  启动后建一次、不随焦点重建；keymap 按内容比较，变了才重发；先发 keymap 再发别的（缺 keymap 就发键是协议错误）；
  modifiers 原样转发，含 NumLock / CapsLock 锁定位，建好时先同步一次，避免应用里 NumLock 失步。
- **抬键跟着按下走。** 每个键码记住按下那次是转发了还是吃掉了：转发过的抬键一定转发，哪怕 Server 对抬起回 `Consumed`——Shift 单击切中 / 英就是这样
  （Shift 按下总是放行，抬起时切换），不转的话应用里 Shift 卡住；按下报过 Server 的抬键照样报，切换就靠它。grab 之前按下的键，抬起时只转发、不报 Server。
- **按键重复。** 被吃掉的键（组句中按住退格、翻页键）由前端按 `repeat_info` 自己重复，等上一次回包再排下一次；`xkb_keymap_key_repeats` 说不重复的键不重复，rate 为 0 不重复，
  后按下的键接替前一个。重复途中 Server 改回放行，就转发一次按下，把重复交还应用。
- **快捷键冲洗。** 组句中 Server 放行带 Ctrl / Alt / Super 的键时，先 `Deactivate{focus_out: false, client_preedit: false, capability_changed: false}` 拿回原文上屏、清 preedit，
  再 `Focus{true}`，最后才转发这个键——与 IBus 前端同一套（`apps/linux/ibus/src/ibus/engine.rs`），不然 Ctrl+S、Ctrl+Enter 会落在半截拼音上。

## 显示

- **一个 popup 用到进程退出。** wl_surface + `zwp_input_popup_surface_v2`，挂缓冲即显示、挂空缓冲即隐藏（同时 `Renderer::forget()`，下次出现不接着上次的动画）。
  第一轮输入区域设成空，鼠标穿透。
- **位置由合成器定。** Hyprland（`InputMethodPopup.cpp`）把 popup 左上角贴在光标下沿，碰到屏幕底就翻到光标上方，碰到右沿往左推；
  `text_input_rectangle` 是光标相对 popup 的矩形，从它能看出翻没翻。渲染器的位图四周带二三十点的阴影边，原样贴上去候选框会离光标很远，
  所以左边与上边的阴影裁到只剩 4 点（右下的阴影保留）；翻到光标上方时底边阴影仍在，候选框与光标之间会空出一截，按 `text_input_rectangle` 调排布留到 P2。
- **缩放。** 有 `wp_fractional_scale_v1` 就按它给的缩放出位图，`wp_viewport` 设逻辑尺寸、`buffer_scale` 保持 1（Hyprland 拿「viewport 尺寸 ÷ buffer_scale」当 popup 尺寸去摆位置）；
  逻辑尺寸取 `ceil(像素 ÷ 缩放)`，位图透明补边到 `round(逻辑 × 缩放)`，保证 1:1 像素不糊。没有分数缩放就退回整数缩放。缩放变了按新缩放重画当前帧。
- **像素格式。** 渲染器出预乘 RGBA；合成器支持 `ABGR8888`（内存序 R,G,B,A）就直接拷，否则换成 `ARGB8888`（B,G,R,A，与 Windows 那份换法相同）。
- **展示回执。** 贴图之后回 `DisplayAcknowledged`（与 Fcitx5 插件一样不等合成器真画出来）：从 `Rendered::hits` 里的译词区域取（候选, 义项）对，最多 128 对（多了 Server 断开连接）。
  抬键的回包帧内容多半没变，没变就不重画，回执照发（展示身份换了）。
  fcitx 版只画第一条义项，这里画全部，所以每条都报，生词标记才会照常衰减。
- **字体。** 与 Windows 同一流程：`[general] font` 空用 `FontLibrary::system("zh-CN")`，否则按字族名找文件；Linux 的「按字族名找文件」与清单路径缺失时的兜底走 fontconfig（见 crate-notes 的 bamboo-slip 一节）。
- **主题。** `ThemeLibrary::load($XDG_DATA_HOME/qingjian/themes).resolve(id, 深浅)`，再套用户字号与动画开关；配置文件与主题目录每秒查一次戳，变了就重读。
  外观 `System` 第一轮按浅色（跟随系统要读 xdg-desktop-portal，见分期）。
  Hyprland 的 `decoration:blur:input_methods = true` 会给 popup 加背景模糊，配半透明背景的主题就是毛玻璃效果。

## 会话与隐私

- 第一轮一个连接一个会话，`app` 报空：`[apps]` 按应用设置不生效，中 / 英状态全局共用，英文候选不出（Server 对没报应用名的会话按不给处理）。
- 密码框（见协议对照）先报 `Capabilities{password: true}` 让 Server 丢弃输入、进隐私模式，之后不 grab，键盘直接归应用。
- Server 连不上时不 grab；连接出错就清掉 preedit、藏起 popup、release grab，2 秒内不再重试——Server 主线程卡住时连接照样能建，但每个请求都要等满 200 ms 超时，不退避的话每键都卡。
- 日志不记按键、字符、上屏文字与前后文。

## 已知限制

- XWayland 应用、KWin、GNOME 不支持（见上表）。
- 失焦后不能再往原来的输入框上屏：`preedit = "window"`（应用里不放拼音）时，切走窗口会丢掉正在组的句子；fcitx 版此时会上原文。
- 输入法看不到鼠标：Shift+点击做选区时，松开 Shift 仍会切换中 / 英。
- 候选窗位置完全由合成器定；Hyprland 0.56 在分数缩放下有 popup 偏移的报告（omarchy #16374），遇到时先抓 `WAYLAND_DEBUG=1` 分清是合成器还是应用报错光标。

## 分期

- **第一轮**：本文 + 可用原型（上面除「分期」外的全部）。
- **P1 补齐 fcitx 版已有的**：按应用会话（`zwlr_foreign_toplevel_manager_v1` 取活动窗口的 app_id → `OpenSession.app`，context 用 app_id 的哈希，64 字节上限）；
  选区翻译（`surrounding_text` 里的选区 → `Selection`）；中 / 英提示（`KeyResult.english` → `Renderer::render_status` 在 popup 里亮一秒）；
  Server 回包附当前模式（`handle_linux` 加字段，像 `translation_pending` 一样只增不改，Fcitx5 插件不读不受影响）；用 `wayland-server` 写假合成器集成测试。
- **P2 macOS / Windows 级特性**：鼠标点选（`Rendered::hit_at` → `Candidate` / `Page`，先验合成器肯不肯把指针事件给 popup）；
  候选右键菜单（`Renderer::render_menu` 画在同一个 popup 里，与 macOS / Windows 一样自绘；要加 Linux 菜单事件，`LINUX_UI_PROTOCOL` 升版，Fcitx5 / Fcitx4 插件跟着改）；
  外观跟随系统（xdg-desktop-portal `org.freedesktop.appearance` 的 `color-scheme`）；按 `text_input_rectangle` 判断翻没翻；托盘（StatusNotifierItem + DBusMenu，原生菜单）；键位提示画进同一个 popup。
- **P3 发布**：`install.sh --frontend wayland`、systemd 用户服务（`PartOf=graphical-session.target`）、deb / rpm、CI、设置程序支持表区分两个版本、用户文档。

## 验证

能自动跑的都是纯逻辑（按键表、抬键与重复、`content_type` 映射、偏移换算与前后文截取、4 KB 切块、像素换算与补边、回包身份判断），不要合成器，CI 照跑。
只能真会话验的，在 Hyprland 上逐个应用过：foot / kitty、GTK4、Firefox、Chromium（`--enable-wayland-ime`）、Qt6——
打字上屏、数字选词、翻页、Shift 切换后 Shift 不卡、组句中按住退格、Ctrl+C 先上原文、NumLock、密码框、组句中切窗口、整句重排换帧、1.5 倍缩放不糊、热加载换主题、Server 中途重启。
嵌套 sway 1.12（headless 后端，不开窗口、不碰宿主会话）+ foot 1.28 + wtype 已经过了一遍（2026-10-28）：
连发与按人速打字的行内拼音、空格 / 数字 / 翻页选词、Esc、回车上原文、退格、单击 Shift 切中英、组句中 Ctrl+A 先上原文、空闲时 Ctrl+C 原样交给应用、
1.5 倍缩放、Server 中途停掉键照常直通与重启后自动接上、按住退格的重复与删空后交还应用、组句中切换窗口、改配置换主题（樱花主题）、第二个输入法收到 `unavailable`。
release 版每帧渲染内置主题几毫秒、樱花主题 1.5 倍下 9–21 毫秒。Hyprland 嵌套跑不起来（它要 xdg_wm_base v6、headless 后端没有 GPU 分配器），只能在真会话里验；
foot 不报前后文、内容类型总是 terminal，密码框与前后文要换 GTK4 应用验。真会话的结果见 `apps/linux/wayland/README.md`。
