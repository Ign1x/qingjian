#!/usr/bin/env bash
# //! 预编译包的安装脚本（package.sh 放到包根目录的 install.sh）：解开产品数据，装到用户目录。
# //! 装哪一版由包决定、不问用户：Fcitx5 版装插件；Wayland 原生版装前端与两个 systemd 用户单元。
set -euo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
install_prefix="$HOME/.local"
# package.sh 打 Wayland 原生版的包时把这一行改成 wayland
frontend=fcitx5
while (($#)); do
  case "$1" in
    --prefix) install_prefix=${2:?--prefix 需要路径}; shift 2 ;;
    --help) echo '用法：install.sh [--prefix 绝对用户目录]（装 Fcitx5 版还是 Wayland 版由包决定）'; exit 0 ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done
[[ "$install_prefix" = /* && "$install_prefix" != / ]] || { echo '需要非根绝对安装路径' >&2; exit 2; }

# 字体的判据与 apps/linux/scripts/install.sh 一致（清单对应 crates/bamboo-slip/src/fonts/linux.rs），改那边时两处一起改。
# 清单一个都不在时（NixOS 这类）问 fontconfig，与渲染器自己找字体走的是同一条路。
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
missing=()
command -v python3 >/dev/null 2>&1 || missing+=(python3)
# Wayland 前端运行时按 libxkbcommon.so.0 这个名字 dlopen（xkbcommon-dl），编译时不检查，只能在这里看。
# 缺省的库路径里没有、只靠 LD_LIBRARY_PATH 找得到时（NixOS），记下那个目录写进服务单元，systemd 起的前端才找得到。
xkbcommon_dir=
if [[ "$frontend" == wayland ]]; then
  any_file /usr/share/fonts/truetype/noto/NotoSans-Regular.ttf /usr/share/fonts/noto/NotoSans-Regular.ttf \
    '/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf' /usr/share/fonts/google-noto/NotoSans-Regular.ttf \
    /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf /usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf \
    /usr/local/share/fonts/noto/NotoSans-Regular.ttf /usr/local/share/fonts/dejavu/DejaVuSans.ttf \
    || fontconfig_matches sans-serif \
    || missing+=("系统 UI 字体（Noto Sans 或 DejaVu）")
  # 候选窗口由前端自己画，没有中文字体候选全是方框，所以算缺依赖
  any_file /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc /usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc \
    /usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc /usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc \
    /usr/local/share/fonts/noto/NotoSansCJKsc-Regular.otf \
    || fontconfig_has ':lang=zh-cn' \
    || missing+=("中文字体（Noto Sans CJK）")
  any_file /usr/share/fonts/truetype/noto/NotoColorEmoji.ttf /usr/share/fonts/noto/NotoColorEmoji.ttf \
    /usr/share/fonts/google-noto-color-emoji-fonts/NotoColorEmoji.ttf /usr/local/share/fonts/noto/NotoColorEmoji.ttf \
    || fontconfig_has ':lang=und-zsye' \
    || echo '提示：没找到 emoji 字体（Noto Color Emoji），表情会画不出来' >&2
  if command -v python3 >/dev/null 2>&1 && ! env -u LD_LIBRARY_PATH python3 -c 'import ctypes; ctypes.CDLL("libxkbcommon.so.0")' 2>/dev/null; then
    IFS=: read -ra library_dirs <<<"${LD_LIBRARY_PATH:-}"
    for dir in "${library_dirs[@]}"; do
      if [[ "$dir" = /* && -e "$dir/libxkbcommon.so.0" ]]; then xkbcommon_dir=$dir; break; fi
    done
    [[ -n "$xkbcommon_dir" ]] || missing+=(libxkbcommon)
  fi
else
  command -v fcitx5 >/dev/null 2>&1 || missing+=(fcitx5)
fi
if ((${#missing[@]})); then
  echo "缺少依赖：${missing[*]}" >&2
  if [[ "$frontend" == wayland ]]; then
    echo 'Debian / Ubuntu：sudo apt install python3 libxkbcommon0 fonts-noto-core fonts-noto-cjk fonts-noto-color-emoji' >&2
    echo 'Arch：sudo pacman -S --needed python libxkbcommon noto-fonts noto-fonts-cjk noto-fonts-emoji' >&2
    echo 'NixOS 上 libxkbcommon 要经 LD_LIBRARY_PATH 给出，见 apps/linux/wayland/README.md' >&2
  else
    echo 'Debian / Ubuntu：sudo apt install python3 fcitx5 fcitx5-frontend-gtk3 fcitx5-frontend-gtk4 fcitx5-frontend-qt6 fcitx5-config-qt' >&2
  fi
  exit 1
fi
install_prefix=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "$install_prefix")
mkdir -p "$here/data"
# 数据包在 macOS 上打，带 LIBARCHIVE.xattr 扩展头，GNU tar 会逐个警告
tar --warning=no-unknown-keyword -xzf "$here/target/release-data/qingjian-data.tar.gz" -C "$here"
files_args=(install "$install_prefix" --root "$here" --server "$here/bin/qingjian-linux-server" --settings "$here/bin/qingjian-settings")
staged=$(mktemp -d)
trap 'rm -rf -- "$staged"' EXIT
if [[ "$frontend" == wayland ]]; then
  # 单元里的可执行文件路径要按这次装的 prefix 填，所以包里的两个模板到安装时才展开
  sed "s|@EXEC@|$install_prefix/bin/qingjian-linux-server|" "$here/qingjian-server.service.in" > "$staged/qingjian-server.service"
  wayland_unit=(-e "s|@EXEC@|$install_prefix/bin/qingjian-linux-wayland|")
  [[ -z "$xkbcommon_dir" ]] || wayland_unit+=(-e "s|^\[Service\]\$|[Service]\nEnvironment=LD_LIBRARY_PATH=$xkbcommon_dir|")
  sed "${wayland_unit[@]}" "$here/qingjian-wayland.service.in" > "$staged/qingjian-wayland.service"
  files_args+=(--wayland "$here/bin/qingjian-linux-wayland" --service "$staged/qingjian-server.service" --wayland-service "$staged/qingjian-wayland.service")
else
  files_args+=(--plugin "$here/lib/qingjian.so")
fi
python3 "$here/files.py" "${files_args[@]}"

if [[ "$frontend" != wayland ]]; then
  printf '已安装。手动启动：%s/bin/qingjian-linux-server\n重启 Fcitx5，在配置工具取消“仅显示当前语言”后添加“青简”。\n设置：应用菜单里的“青简设置”，或 %s/bin/qingjian-settings\n' "$install_prefix" "$install_prefix"
  exit 0
fi
printf '已安装：Wayland 前端 %s/bin/qingjian-linux-wayland、Server %s/bin/qingjian-linux-server、设置程序 %s/bin/qingjian-settings\n' \
  "$install_prefix" "$install_prefix" "$install_prefix"
# systemd 只从 ~/.config/systemd/user 找用户单元：配置目录被换到别处时（测试、临时安装）单元装在那里它看不见，
# 按名字 enable 反而会去开本机真装的那一份，所以这种情况只提示手动启动
systemd_sees_unit=false
[[ "${XDG_CONFIG_HOME:-$HOME/.config}" == "$HOME/.config" ]] && systemd_sees_unit=true
if command -v systemctl >/dev/null 2>&1 && [[ "$systemd_sees_unit" == true ]]; then
  echo '包不会替你启用（同一个 seat 只能有一个输入法）。第一次装好之后：'
  echo '  systemctl --user daemon-reload && systemctl --user enable --now qingjian-server.service qingjian-wayland.service'
  echo '  niri（niri-session）、uwsm 这类会启动 graphical-session.target 的会话随登录起来；Hyprland / sway 直接启动的'
  echo '  要在配置里加一行（sway 写 exec，不是 exec-once）：'
  echo '    exec-once = systemctl --user import-environment WAYLAND_DISPLAY && systemctl --user start qingjian-wayland.service'
else
  echo "Server 要手动起：$install_prefix/bin/qingjian-linux-server"
  echo "Wayland 前端也要手动起：$install_prefix/bin/qingjian-linux-wayland"
  [[ "$systemd_sees_unit" == true ]] || echo 'XDG_CONFIG_HOME 不是 ~/.config，systemd 看不到这份单元，没有给 enable 命令。'
fi
echo '关掉 fcitx5（或它的「Wayland 输入法前端」附加组件）：它开着时前端起不来（日志里写明 seat 被占）。'
[[ -z "$xkbcommon_dir" ]] \
  || echo "libxkbcommon 取自 LD_LIBRARY_PATH 里的 $xkbcommon_dir：服务单元里已经带上，直接启动前端时也要带上；这个目录没了前端就起不来。"
echo '起不来的原因：journalctl --user -u qingjian-wayland；说明见官网文档「Linux」的 Wayland 原生版一节。'
