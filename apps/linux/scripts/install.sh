#!/usr/bin/env bash
# //! 用户目录安装：按这台机器上有哪个输入法框架装对应的前端（fcitx5 插件 / IBus 前端），Server 两边共用。
# //! 装哪几支由 --frontend 决定，缺省 auto = 能装的都装；只有 Fcitx4 的系统（Ubuntu 22.04）用 --fcitx4。
# //! Wayland 原生版（自己当输入法、候选窗自绘）不进 auto：同一个 seat 只能有一个输入法，要用 --frontend wayland 明确选。
# //! 不碰系统目录，也不碰用户数据。
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
install_prefix="$HOME/.local"
build_profile=release
cmake_type=Release
sample=false
frontend=auto
frontend_given=false
fcitx4=false
enable_service=false
while (($#)); do
  case "$1" in
    --prefix) install_prefix=${2:?--prefix 需要路径}; shift 2 ;;
    --frontend) frontend=${2:?--frontend 需要 auto/fcitx5/ibus/both/wayland}; frontend_given=true; shift 2 ;;
    --fcitx4) fcitx4=true; shift ;;
    --debug) build_profile=debug; cmake_type=Debug; shift ;;
    --sample) sample=true; shift ;;
    --enable-service) enable_service=true; shift ;;
    --help)
      cat <<'USAGE'
用法：install.sh [选项]
  --prefix 目录        安装到哪（缺省 ~/.local）
  --frontend 值        auto（缺省，能装的 fcitx 版前端都装）/ fcitx5 / ibus / both /
                       wayland（Wayland 原生版：Hyprland、sway、niri 这类桌面），可用逗号连写，如 fcitx5,wayland
  --fcitx4             只装 Fcitx4 插件（Ubuntu 22.04 这类只有 Fcitx4 的系统），不能与 --frontend 同用
  --debug              装 debug 构建
  --sample             只装样例词库，不要产品数据
  --enable-service     顺手 systemctl --user enable --now 把 Server（装了 Wayland 原生版时连同它）挂上自启
USAGE
      exit 0 ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done
[[ "$install_prefix" = /* && "$install_prefix" != / ]] || { echo '需要非根绝对安装路径' >&2; exit 2; }
# 点名的那几支缺东西要报错；auto 只装能装的、跳过的说出来
ask_auto=false
ask_fcitx5=false
ask_ibus=false
want_wayland=false
IFS=, read -ra frontend_parts <<<"$frontend"
((${#frontend_parts[@]})) || frontend_parts=('')
for part in "${frontend_parts[@]}"; do
  case "$part" in
    auto) ask_auto=true ;;
    fcitx5) ask_fcitx5=true ;;
    ibus) ask_ibus=true ;;
    both) ask_fcitx5=true; ask_ibus=true ;;
    wayland) want_wayland=true ;;
    *) echo "--frontend 只认 auto / fcitx5 / ibus / both / wayland（可用逗号连写）：$part" >&2; exit 2 ;;
  esac
done
if [[ "$want_wayland" == true && "$(uname -s)" != Linux ]]; then
  echo 'Wayland 原生版只支持 Linux' >&2
  exit 2
fi
if $fcitx4 && $frontend_given; then
  echo '--fcitx4 与 --frontend 不能同用：Fcitx4 与 Fcitx5 / IBus 装在不同的系统上' >&2
  exit 2
fi

# fcitx5 那一支的插件是 C++，要编译器与 fcitx5 的开发包；缺了只能说清楚缺什么。
fcitx5_missing() {
  local missing=() tool module
  for tool in cmake c++ pkg-config; do
    command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
  done
  if command -v pkg-config >/dev/null 2>&1; then
    for module in Fcitx5Core Fcitx5Utils Fcitx5Config nlohmann_json; do
      pkg-config --exists "$module" || missing+=("$module")
    done
  fi
  echo "${missing[*]:-}"
}

# Fcitx4 插件同样是 C++，要的是 fcitx4 与 X11 的开发包。
fcitx4_missing() {
  local missing=() tool module
  for tool in cmake c++ pkg-config; do
    command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
  done
  if command -v pkg-config >/dev/null 2>&1; then
    for module in fcitx x11 xres nlohmann_json; do
      pkg-config --exists "$module" || missing+=("$module")
    done
  fi
  echo "${missing[*]:-}"
}

# IBus 那一支是纯 Rust，编译不缺东西；装了没用才是问题，所以看的是机器上有没有 ibus。
ibus_missing() {
  command -v ibus-daemon >/dev/null 2>&1 || echo ibus-daemon
}

# 现在这个会话实际在用哪个框架。**只用来提示**，不决定装什么：
# 装完再切框架是常事，两支都装着不冲突（Server 按连接隔离会话）。
current_framework() {
  local hint="${XMODIFIERS:-}${GTK_IM_MODULE:-}${QT_IM_MODULE:-}"
  case "$hint" in
    *fcitx*) echo fcitx5 ;;
    *ibus*) echo ibus ;;
    *)
      # GNOME 会话里内建的就是 IBus，即使这几个变量都没设
      case "${XDG_CURRENT_DESKTOP:-}" in *GNOME*) echo ibus ;; *) echo 未知 ;; esac ;;
  esac
}

# 这个会话的桌面能不能用 Wayland 原生版，也只用来提示。要合成器给 input-method-v2：
# KWin 只有 v1、GNOME 没有（docs/design/linux-wayland.md 的表）；表外的桌面不猜。
wayland_desktop() {
  [[ -n "${WAYLAND_DISPLAY:-}" ]] || { echo 否; return; }
  case "${XDG_CURRENT_DESKTOP:-}" in
    *GNOME*|*KDE*) echo 不能 ;;
    *Hyprland*|*sway*|*niri*|*labwc*|*wlroots*|*mango*) echo 能 ;;
    *) echo 未知 ;;
  esac
}

want_fcitx4=false
if $fcitx4; then
  fcitx4_gap=$(fcitx4_missing)
  if [[ -n "$fcitx4_gap" ]]; then
    echo "装不了 Fcitx4 插件，缺：$fcitx4_gap" >&2
    echo 'Debian / Ubuntu：sudo apt install cmake g++ pkg-config fcitx-libs-dev libx11-dev libxres-dev nlohmann-json3-dev' >&2
    exit 1
  fi
  want_fcitx4=true
  # 下面那段按 Fcitx5 / IBus 挑前端，Fcitx4 都不装
  ask_auto=false
fi
fcitx5_gap=$(fcitx5_missing)
ibus_gap=$(ibus_missing)
in_use=$(current_framework)
desktop=$(wayland_desktop)
want_fcitx5=$ask_fcitx5
want_ibus=$ask_ibus
if [[ "$ask_auto" == true ]]; then
  [[ -n "$fcitx5_gap" ]] || want_fcitx5=true
  [[ -n "$ibus_gap" ]] || want_ibus=true
fi
if [[ "$ask_fcitx5" == true && -n "$fcitx5_gap" ]]; then
  echo "装不了 fcitx5 那一支，缺：$fcitx5_gap" >&2
  echo 'Debian / Ubuntu：sudo apt install cmake g++ pkg-config libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev nlohmann-json3-dev' >&2
  echo 'FreeBSD：pkg install bash cmake pkgconf python3 fcitx5 nlohmann-json rustup-init（rustup-init 装 Rust）' >&2
  exit 1
fi
if [[ "$ask_ibus" == true && -n "$ibus_gap" ]]; then
  echo "装不了 IBus 那一支，缺：$ibus_gap（Debian / Ubuntu：sudo apt install ibus）" >&2
  exit 1
fi
if [[ "$want_fcitx5" == false && "$want_ibus" == false && "$want_fcitx4" == false && "$want_wayland" == false ]]; then
  echo '两个框架都没找到：fcitx5 缺开发包、机器上也没有 ibus-daemon。' >&2
  echo '装上其中一个再来，或用 --frontend 明确指定要装哪一支（Hyprland、sway、niri 这类桌面可用 --frontend wayland）。' >&2
  exit 1
fi
# auto 跳过某一支要说出来，不然「装好了」与「那一支根本没装」看起来一模一样。
if [[ "$ask_auto" == true ]]; then
  [[ "$want_fcitx5" == true ]] || echo "跳过 fcitx5 那一支（缺：$fcitx5_gap）"
  [[ "$want_ibus" == true ]] || echo "跳过 IBus 那一支（缺：$ibus_gap）"
  [[ "$want_wayland" == true || "$desktop" != 能 ]] \
    || echo "这个桌面（${XDG_CURRENT_DESKTOP:-}）也能用 Wayland 原生版：候选窗口由青简绘制，--frontend wayland 安装"
fi
# 最容易踩的一脚：装完一切正常，唯独正在用的那个框架什么都没装上。
# 装 Wayland 原生版的本来就要把 fcitx5 关掉（装完会提示），不在这里报。
if [[ "$in_use" == fcitx5 && "$want_fcitx5" == false && "$want_fcitx4" == false && "$want_wayland" == false ]]; then
  echo "注意：这个会话在用 fcitx5，但这次没装 fcitx5 那一支（缺：$fcitx5_gap）" >&2
fi
if [[ "$in_use" == ibus && "$want_ibus" == false && "$want_fcitx4" == false && "$want_wayland" == false ]]; then
  echo "注意：这个会话在用 IBus，但这次没装 IBus 那一支（缺：$ibus_gap）" >&2
fi
if [[ "$want_wayland" == true && "$desktop" == 不能 ]]; then
  echo "注意：这个会话是 ${XDG_CURRENT_DESKTOP:-}，它的合成器不提供 Wayland 原生版要的输入法接口，这里要用 fcitx 版（--frontend auto）" >&2
fi

missing=()
for tool in cargo python3; do
  command -v "$tool" >/dev/null 2>&1 || missing+=("$tool")
done
command -v pkg-config >/dev/null 2>&1 && { pkg-config --exists openssl || missing+=(openssl); }
# 路径与 crates/bamboo-slip/src/fonts/linux.rs 的清单对应，改那边时同步这里。清单一个都不在时（NixOS 这类）
# 它问 fontconfig：界面字体要 fc-match 给得出文件，中文与 emoji 要 fc-list 按语言列得出来，这里同样问。
# 没有系统 UI 字体设置程序起不来，算缺依赖；中文与 emoji 字体缺了照样能起，只提示——
# Wayland 原生版的候选窗口是自己画的，没有中文字体候选全是方框，所以装它时中文字体也算缺依赖。
any_file() {
  local path
  for path in "$@"; do [[ -f "$path" ]] && return 0; done
  return 1
}
fontconfig_has() {
  command -v fc-list >/dev/null 2>&1 && [[ -n "$(fc-list --format '%{file}\n' "$1" 2>/dev/null)" ]]
}
fontconfig_matches() {
  local file
  command -v fc-match >/dev/null 2>&1 && file=$(fc-match --format '%{file}' "$1" 2>/dev/null) && [[ -f "$file" ]]
}
any_file /usr/share/fonts/truetype/noto/NotoSans-Regular.ttf /usr/share/fonts/noto/NotoSans-Regular.ttf \
  '/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf' /usr/share/fonts/google-noto/NotoSans-Regular.ttf \
  /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf /usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf \
  /usr/local/share/fonts/noto/NotoSans-Regular.ttf /usr/local/share/fonts/dejavu/DejaVuSans.ttf \
  || fontconfig_matches sans-serif \
  || missing+=("系统 UI 字体（Noto Sans 或 DejaVu）")
if ! any_file /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc /usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc \
  /usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc /usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc \
  /usr/local/share/fonts/noto/NotoSansCJKsc-Regular.otf \
  && ! fontconfig_has ':lang=zh-cn'; then
  if [[ "$want_wayland" == true ]]; then
    missing+=("中文字体（Noto Sans CJK）")
  else
    echo '提示：没找到中文字体（Noto Sans CJK），设置程序里的中文会画不出来' >&2
  fi
fi
any_file /usr/share/fonts/truetype/noto/NotoColorEmoji.ttf /usr/share/fonts/noto/NotoColorEmoji.ttf \
  /usr/share/fonts/google-noto-color-emoji-fonts/NotoColorEmoji.ttf /usr/local/share/fonts/noto/NotoColorEmoji.ttf \
  || fontconfig_has ':lang=und-zsye' \
  || echo '提示：没找到 emoji 字体（Noto Color Emoji），表情会画不出来' >&2
# Wayland 前端运行时按 libxkbcommon.so.0 这个名字 dlopen libxkbcommon（xkbcommon-dl），编译时不检查，只能在这里看。
# 缺省的库路径里没有、只靠 LD_LIBRARY_PATH 找得到时（NixOS），把那个目录记下来写进服务单元，systemd 起的前端才找得到。
xkbcommon_dir=
if [[ "$want_wayland" == true ]] && command -v python3 >/dev/null 2>&1 \
  && ! env -u LD_LIBRARY_PATH python3 -c 'import ctypes; ctypes.CDLL("libxkbcommon.so.0")' 2>/dev/null; then
  IFS=: read -ra library_dirs <<<"${LD_LIBRARY_PATH:-}"
  for dir in "${library_dirs[@]}"; do
    if [[ "$dir" = /* && -e "$dir/libxkbcommon.so.0" ]]; then xkbcommon_dir=$dir; break; fi
  done
  [[ -n "$xkbcommon_dir" ]] || missing+=(libxkbcommon)
fi
if ((${#missing[@]})); then
  echo "缺少依赖：${missing[*]}" >&2
  if [[ "$want_wayland" == true ]]; then
    echo 'Debian / Ubuntu：sudo apt install python3 pkg-config libssl-dev libxkbcommon0 fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji' >&2
    echo 'Arch：sudo pacman -S --needed base-devel python openssl libxkbcommon noto-fonts noto-fonts-cjk noto-fonts-emoji' >&2
    echo 'Rust 见 https://rustup.rs；NixOS 上 libxkbcommon 要经 LD_LIBRARY_PATH 给出，见 apps/linux/wayland/README.md' >&2
  else
    echo 'Debian / Ubuntu：sudo apt install python3 pkg-config libssl-dev fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji；Rust 见 https://rustup.rs' >&2
  fi
  exit 1
fi

# FreeBSD 的 realpath 没有 -m，用 python3 解析（上面已经确认有 python3）
install_prefix=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$install_prefix")
cargo_output=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "${CARGO_TARGET_DIR:-$repo_root/target}")
cargo_args=(build --manifest-path "$repo_root/Cargo.toml" --target-dir "$cargo_output" -p qingjian-linux-server -p qingjian-settings --locked)
[[ "$want_ibus" != true ]] || cargo_args+=(-p qingjian-linux-ibus)
[[ "$build_profile" != release ]] || cargo_args+=(--release)
cargo "${cargo_args[@]}"
if [[ "$want_wayland" == true ]]; then
  # 单独一次 cargo：与设置程序一起编时 cargo 合并依赖的特性，winit 会把 wayland-backend 换成 dlopen libwayland 的那种
  # （没有 libwayland-client.so 的机器上起不来）；单独编是纯 Rust 的那种，只多 dlopen 一个 libxkbcommon
  wayland_args=(build --manifest-path "$repo_root/Cargo.toml" --target-dir "$cargo_output" -p qingjian-linux-wayland --locked)
  [[ "$build_profile" != release ]] || wayland_args+=(--release)
  cargo "${wayland_args[@]}"
fi
built="$cargo_output/$build_profile"

files_args=(install "$install_prefix" --root "$repo_root" --server "$built/qingjian-linux-server" --settings "$built/qingjian-settings")
[[ "$sample" != true ]] || files_args+=(--sample)
staged=$(mktemp -d)
trap 'rm -rf -- "$staged"' EXIT

if [[ "$want_fcitx5" == true ]]; then
  cmake_output="$repo_root/target/fcitx5-install-$build_profile"
  cmake -S "$repo_root/apps/linux/fcitx5" -B "$cmake_output" "-DCMAKE_BUILD_TYPE=$cmake_type" -DBUILD_TESTING=OFF
  cmake --build "$cmake_output" --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-2}"
  files_args+=(--plugin "$cmake_output/qingjian.so")
fi
if [[ "$want_fcitx4" == true ]]; then
  cmake_output="$repo_root/target/fcitx4-install-$build_profile"
  cmake -S "$repo_root/apps/linux/fcitx4" -B "$cmake_output" "-DCMAKE_BUILD_TYPE=$cmake_type"
  cmake --build "$cmake_output" --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-2}"
  files_args+=(--plugin "$cmake_output/fcitx-qingjian.so" --flavor fcitx4)
fi
if [[ "$want_ibus" == true ]]; then
  # 组件 XML 里的 <exec> 是装好之后的绝对路径，所以现在就按 prefix 生成
  "$built/qingjian-linux-ibus" --ibus-xml "$install_prefix/bin/qingjian-linux-ibus" > "$staged/qingjian.xml"
  # **IBus 不读 ~/.local/share/ibus/component/**，只认 IBUS_COMPONENT_PATH（没设时只有系统目录），
  # 所以光把 XML 放进用户目录，真会话里根本看不到这个引擎（IBus 1.5.34 上验过）。
  # 用 environment.d 在会话启动时把用户目录加进去；**用户目录放最前面**：两个目录里有同名组件时
  # 以排在前面的为准（也验过），装过旧版 deb 的机器上新装的才会生效。
  data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
  # 只有 IBus 确实装在 /usr/local 下才换过去：装青简时 IBus 可能还没装，那时两处都不存在，
  # 退到 /usr/local 的话之后装上的系统组件（dconf 配置程序）就不在路径里，ibus-daemon 起不来（Arch 上踩过）
  system_components=/usr/share/ibus/component
  [[ ! -d "$system_components" && -d /usr/local/share/ibus/component ]] && system_components=/usr/local/share/ibus/component
  cat > "$staged/qingjian-ibus.conf" <<ENV
# 青简 IBus 前端（apps/linux/scripts/install.sh 生成，uninstall.sh 删除）。
# IBus 不读用户目录下的组件，只认这个变量；用户目录放最前面，同名组件以前面的为准。
IBUS_COMPONENT_PATH=$data_home/ibus/component:\${IBUS_COMPONENT_PATH:-$system_components}
ENV
  files_args+=(--ibus "$built/qingjian-linux-ibus" --ibus-xml "$staged/qingjian.xml" --ibus-env "$staged/qingjian-ibus.conf")
fi
[[ "$want_wayland" != true ]] || files_args+=(--wayland "$built/qingjian-linux-wayland")
if command -v systemctl >/dev/null 2>&1; then
  sed "s|@EXEC@|$install_prefix/bin/qingjian-linux-server|" \
    "$repo_root/apps/linux/scripts/qingjian-server.service.in" > "$staged/qingjian-server.service"
  files_args+=(--service "$staged/qingjian-server.service")
  if [[ "$want_wayland" == true ]]; then
    wayland_unit=(-e "s|@EXEC@|$install_prefix/bin/qingjian-linux-wayland|")
    [[ -z "$xkbcommon_dir" ]] || wayland_unit+=(-e "s|^\[Service\]\$|[Service]\nEnvironment=LD_LIBRARY_PATH=$xkbcommon_dir|")
    sed "${wayland_unit[@]}" "$repo_root/apps/linux/scripts/qingjian-wayland.service.in" > "$staged/qingjian-wayland.service"
    files_args+=(--wayland-service "$staged/qingjian-wayland.service")
  fi
fi

python3 "$repo_root/apps/linux/scripts/files.py" "${files_args[@]}"
# ibus-daemon 的注册表缓存只看缓存里记着的目录变没变，IBUS_COMPONENT_PATH 换了它不管：
# 留着旧缓存，新加的目录（青简）可能看不到，缺了系统目录时 daemon 直接退出。删掉让它按新路径重建。
[[ "$want_ibus" != true ]] || rm -f "${XDG_CACHE_HOME:-$HOME/.cache}/ibus/bus/registry"

echo '已安装：'
[[ "$want_fcitx5" != true ]] || echo "  fcitx5 插件  $install_prefix/lib/fcitx5/qingjian.so"
[[ "$want_fcitx4" != true ]] || echo "  fcitx4 插件  $install_prefix/lib/fcitx/fcitx-qingjian.so"
[[ "$want_ibus" != true ]] || echo "  IBus 前端    $install_prefix/bin/qingjian-linux-ibus"
[[ "$want_wayland" != true ]] || echo "  Wayland 前端 $install_prefix/bin/qingjian-linux-wayland（Wayland 原生版）"
echo "  Server       $install_prefix/bin/qingjian-linux-server"
echo "  设置程序     $install_prefix/bin/qingjian-settings（应用菜单里的「青简设置」）"
# systemd 只从 ~/.config/systemd/user 找用户单元：配置目录被换到别处时（测试、临时安装）单元装在那里它看不见，
# 按名字 enable 反而会去开本机真装的那一份，所以这种情况不 enable、只提示
systemd_sees_unit=false
[[ "${XDG_CONFIG_HOME:-$HOME/.config}" == "$HOME/.config" ]] && systemd_sees_unit=true
by_systemd=false
command -v systemctl >/dev/null 2>&1 && [[ "$systemd_sees_unit" == true ]] && by_systemd=true
if [[ "$by_systemd" == true ]]; then
  if [[ "$enable_service" == true ]]; then
    systemctl --user daemon-reload
    systemctl --user enable --now qingjian-server.service
    echo 'Server 已挂上开机自启（systemctl --user status qingjian-server）。'
    if [[ "$want_wayland" == true ]]; then
      systemctl --user enable --now qingjian-wayland.service
      echo 'Wayland 原生版已登记随 graphical-session.target 启动，也已试着现在启动（systemctl --user status qingjian-wayland 看结果）。'
    fi
  else
    echo "Server 现在还要手动起：$install_prefix/bin/qingjian-linux-server"
    echo '挂开机自启：systemctl --user daemon-reload && systemctl --user enable --now qingjian-server.service'
    [[ "$want_wayland" != true ]] \
      || echo 'Wayland 原生版现在启动：systemctl --user daemon-reload && systemctl --user start qingjian-wayland.service'
  fi
else
  [[ "$systemd_sees_unit" == true || "$enable_service" != true ]] \
    || echo 'XDG_CONFIG_HOME 不是 ~/.config，systemd 看不到这份单元，没有 enable（避免误开本机真装的那一份）。'
  echo "Server 要手动起：$install_prefix/bin/qingjian-linux-server"
  [[ "$want_wayland" != true ]] || echo "Wayland 前端也要手动起：$install_prefix/bin/qingjian-linux-wayland"
fi
[[ "$want_fcitx5" != true ]] || echo 'fcitx5：重启 Fcitx5，在配置工具取消“仅显示当前语言”后添加“青简”。'
[[ "$want_fcitx4" != true ]] || echo 'fcitx4：重启 Fcitx4，在配置工具添加“青简”。'
if [[ "$want_ibus" == true ]]; then
  echo 'IBus：**重新登录一次**（组件路径在会话启动时读），然后在“设置 → 键盘”里添加“青简”，或 ibus engine qingjian。'
  echo '      IBus 那一支只是前端，要 Server 起着才能打字；没加 --enable-service 的话记得按上面的命令挂自启。'
  if [[ -f /usr/share/ibus/component/qingjian.xml ]]; then
    echo '注意：系统里还装着另一份青简（/usr/share/ibus/component/qingjian.xml，多半是旧版 deb）。'
    echo '      重新登录后以这次装的为准；想回到那一份就跑 uninstall.sh。'
  fi
fi
if [[ "$want_wayland" == true ]]; then
  echo 'Wayland 原生版：'
  echo '  同一个 seat 只能有一个输入法，fcitx5 开着时它起不来（日志里写明 seat 被占）。二选一：'
  echo '    不再用 fcitx5：去掉合成器配置里启动它的那一行（或它的自启动项），重新登录；'
  echo '    还要用 fcitx5：在它的配置工具的附加组件里关掉「Wayland 输入法前端」（waylandim）。'
  ! pgrep -u "$(id -u)" -x fcitx5 >/dev/null 2>&1 || echo '  注意：fcitx5 现在正开着。' >&2
  if [[ "$by_systemd" == true ]]; then
    # Hyprland、sway 直接启动时会话里没有 graphical-session.target，enable 了也不会随登录起来；
    # 先导入 WAYLAND_DISPLAY 再起，不然与合成器自己导入环境的那一下赛跑，输了单元的条件不满足、不起
    echo '  随桌面启动：niri（用 niri-session 登录）、uwsm 这类会启动 graphical-session.target 的会话里'
    echo '    systemctl --user enable qingjian-wayland.service（--enable-service 就是做这个）；'
    echo '  Hyprland / sway 直接启动的，在配置里加一行（sway 写 exec，不是 exec-once）：'
    echo '    exec-once = systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service'
  else
    echo "  随桌面启动：在合成器的自启动配置里先后启动 $install_prefix/bin/qingjian-linux-server"
    echo "    与 $install_prefix/bin/qingjian-linux-wayland"
  fi
  echo '  应用：GTK_IM_MODULE / QT_IM_MODULE 不要设成 fcitx / ibus；Chromium / Electron 加 --enable-wayland-ime；XWayland 应用没有输入法。'
  for variable in GTK_IM_MODULE QT_IM_MODULE; do
    case "${!variable:-}" in
      fcitx* | ibus*) echo "  注意：现在 $variable=${!variable}，这类应用会绕过 Wayland 原生版；去掉它后重新登录。" >&2 ;;
    esac
  done
  if [[ -n "$xkbcommon_dir" ]]; then
    echo "  libxkbcommon 取自 LD_LIBRARY_PATH 里的 $xkbcommon_dir：服务单元里已经带上，直接启动前端时也要带上；"
    echo '    这个目录没了前端就起不来（NixOS 上给它建一个 GC root，见 apps/linux/wayland/README.md）。'
  fi
  echo '  起不来的原因：journalctl --user -u qingjian-wayland；说明见官网文档「Linux」的 Wayland 原生版一节。'
fi
