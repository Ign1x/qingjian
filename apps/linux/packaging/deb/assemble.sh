#!/usr/bin/env bash
# 在 Debian 13 容器里跑（build.sh 调，仓库挂在 /src）：编 fcitx5 插件，从二进制推出依赖，拼三个 .deb：
# - qingjian：Server、IBus 前端、词库、systemd 用户服务（build.sh 已摆好在 $stage/qingjian）
# - qingjian-fcitx5：fcitx5 插件
# - qingjian-wayland：Wayland 原生版的前端与它的 systemd 用户服务（build.sh 已摆好在 $stage/qingjian-wayland）
set -euo pipefail
stage=/src/target/linux-pkg/deb
version=${DEB_VERSION:?}
arch=$(dpkg --print-architecture)
multiarch=$(dpkg-architecture -qDEB_HOST_MULTIARCH)
maintainer='Qingjian <https://qingjian.app>'

build="$stage/fcitx5-build"
cmake -S /src/apps/linux/fcitx5 -B "$build" -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF >/dev/null
cmake --build "$build" --parallel >/dev/null
fcitx="$stage/qingjian-fcitx5"
# 插件放 fcitx5 自己的插件目录（Debian 是多架构路径），描述文件里 Library=qingjian 按名字找
install -Dm644 "$build/qingjian.so" "$fcitx/usr/lib/$multiarch/fcitx5/qingjian.so"
strip --strip-unneeded "$fcitx/usr/lib/$multiarch/fcitx5/qingjian.so"
install -Dm644 /src/apps/linux/fcitx5/data/addon/qingjian.conf "$fcitx/usr/share/fcitx5/addon/qingjian.conf"
install -Dm644 /src/apps/linux/fcitx5/data/inputmethod/qingjian.conf "$fcitx/usr/share/fcitx5/inputmethod/qingjian.conf"
install -Dm644 /src/LICENSE "$fcitx/usr/share/doc/qingjian-fcitx5/copyright"

core="$stage/qingjian"
strip --strip-unneeded "$core/usr/bin/qingjian-linux-server" "$core/usr/bin/qingjian-linux-ibus"
wayland="$stage/qingjian-wayland"
strip --strip-unneeded "$wayland/usr/bin/qingjian-linux-wayland"

# 动态库依赖从二进制推（libc6 的最低版本、libfcitx5core7 这些），不手写；
# dpkg-shlibdeps 只肯在带 debian/control 的目录里跑，给它一个最小的
shlibs() {
  local dir
  dir=$(mktemp -d)
  mkdir "$dir/debian"
  printf 'Source: qingjian\n\nPackage: qingjian\nArchitecture: any\n' > "$dir/debian/control"
  (cd "$dir" && dpkg-shlibdeps -O "$@" | sed -n 's/^shlibs:Depends=//p')
  rm -rf "$dir"
}

# 权限归一：宿主机 umask 是 002 时拷出来的是 664 / 775
normalize() {
  find "$1" -type d -exec chmod 755 {} +
  find "$1" -type f ! -path '*/usr/bin/*' -exec chmod 644 {} +
}

control() {
  local dir=$1 name=$2 depends=$3 extra=$4 summary=$5 description=$6
  mkdir -p "$dir/DEBIAN"
  cat > "$dir/DEBIAN/control" <<CONTROL
Package: $name
Version: $version
Architecture: $arch
Maintainer: $maintainer
Installed-Size: $(du -sk --exclude=DEBIAN "$dir" | cut -f1)
Depends: $depends
${extra}Section: utils
Priority: optional
Homepage: https://qingjian.app
Description: $summary
$description
CONTROL
}

normalize "$core"
control "$core" qingjian "$(shlibs "$core/usr/bin/qingjian-linux-server" "$core/usr/bin/qingjian-linux-ibus")" \
  $'Recommends: ibus | qingjian-fcitx5 | qingjian-wayland\n' \
  '青简输入法：Server、IBus 前端与词库' \
  ' 跨平台拼音输入法。本包是引擎（Server，systemd 用户服务）、IBus 前端与随包词库；
 用 fcitx5 的再装 qingjian-fcitx5，Hyprland、sway、niri 这类桌面可改装 qingjian-wayland。
 Server 装完就起；IBus 要重新登录一次（或 ibus restart）才看得到青简。'
# Server 挂到每个用户的会话里自启。--global 只对之后起来的用户 systemd 实例生效，而注销再登录得快时
# 实例根本不重起（Ubuntu 24.04 上踩过：装完重登，Server 一次都没起）；所以已经登录的用户直接在他们
# 正在跑的实例里重载并（重）启动：装完不用重登就能打字，升级也换上新的 Server
cat > "$core/DEBIAN/postinst" <<'SCRIPT'
#!/bin/sh
set -e
if [ "$1" = configure ] && command -v systemctl >/dev/null 2>&1; then
  systemctl --global enable qingjian-server.service >/dev/null 2>&1 || true
  # 用户目录里的同名单元（源码安装的 install.sh 登记的）会盖过这个包的 Server：
  # 前端用这个包的、Server 用那边的，版本一错开就打不出字（本机踩过：Server 断开会话，按键全部放行）
  getent passwd | while IFS=: read -r name _ uid _ _ home _; do
    [ "$uid" -ge 1000 ] 2>/dev/null || continue
    if [ -f "$home/.config/systemd/user/qingjian-server.service" ]; then
      # 拆成短行：dnf5 按终端宽度截断装包脚本的输出
      echo "注意：用户 $name 有自己的青简 Server 服务，会盖过这个包装的：" >&2
      echo "  $home/.config/systemd/user/qingjian-server.service" >&2
      echo "  以该用户卸掉那一份——源码安装的：apps/linux/scripts/uninstall.sh" >&2
    fi
  done
  loginctl list-users --no-legend 2>/dev/null | while read -r _ user _; do
    systemctl --user -M "$user@" daemon-reload >/dev/null 2>&1 || true
    systemctl --user -M "$user@" restart qingjian-server.service >/dev/null 2>&1 || true
  done
fi
SCRIPT
cat > "$core/DEBIAN/prerm" <<'SCRIPT'
#!/bin/sh
set -e
if [ "$1" = remove ] && command -v systemctl >/dev/null 2>&1; then
  systemctl --global disable qingjian-server.service >/dev/null 2>&1 || true
  loginctl list-users --no-legend 2>/dev/null | while read -r _ user _; do
    systemctl --user -M "$user@" stop qingjian-server.service >/dev/null 2>&1 || true
  done
fi
SCRIPT
chmod 755 "$core/DEBIAN/postinst" "$core/DEBIAN/prerm"

normalize "$fcitx"
control "$fcitx" qingjian-fcitx5 "qingjian (= $version), fcitx5, $(shlibs "$fcitx/usr/lib/$multiarch/fcitx5/qingjian.so")" '' \
  '青简输入法的 fcitx5 插件' \
  ' 让 fcitx5 能用青简；引擎在 qingjian 包里。装完重启 fcitx5，在配置工具里添加「青简」。'

normalize "$wayland"
# libxkbcommon 是前端运行时 dlopen 的，dpkg-shlibdeps 看不到，手写；字体只是候选窗画得好不好，放 Recommends
control "$wayland" qingjian-wayland "qingjian (= $version), libxkbcommon0, $(shlibs "$wayland/usr/bin/qingjian-linux-wayland")" \
  $'Recommends: fonts-noto-core | fonts-dejavu-core, fonts-noto-cjk, fonts-noto-color-emoji, fontconfig\n' \
  '青简输入法 Wayland 原生版' \
  ' 不经过 Fcitx / IBus，青简自己当输入法，候选窗口自绘，主题、字体与字号都生效；
 用于 Hyprland、sway（1.10 起）、niri、labwc 等桌面，KDE 与 GNOME 用 qingjian-fcitx5 或 IBus。
 装完不会自己启用：要用的用户执行 systemctl --user enable --now qingjian-wayland.service，
 并关掉 fcitx5（或它的 Wayland 输入法前端），同一时间只能有一个输入法。'
# 不 --global enable：同一个 seat 只能有一个输入法，GNOME / KDE 上它也起不来，要用的用户自己 enable。
# 已经登录、正开着它的用户换上新的（try-restart 只重启在跑的）；第一次装时说一声怎么启用
cat > "$wayland/DEBIAN/postinst" <<'SCRIPT'
#!/bin/sh
set -e
if [ "$1" = configure ] && command -v systemctl >/dev/null 2>&1; then
  loginctl list-users --no-legend 2>/dev/null | while read -r _ user _; do
    systemctl --user -M "$user@" daemon-reload >/dev/null 2>&1 || true
    systemctl --user -M "$user@" try-restart qingjian-wayland.service >/dev/null 2>&1 || true
  done
  if [ -z "$2" ]; then
    echo "青简 Wayland 原生版：要用的用户执行" >&2
    echo "  systemctl --user enable --now qingjian-wayland.service" >&2
    echo "  并关掉 fcitx5（或它的 Wayland 输入法前端）" >&2
  fi
fi
SCRIPT
cat > "$wayland/DEBIAN/prerm" <<'SCRIPT'
#!/bin/sh
set -e
if [ "$1" = remove ] && command -v systemctl >/dev/null 2>&1; then
  systemctl --global disable qingjian-wayland.service >/dev/null 2>&1 || true
  loginctl list-users --no-legend 2>/dev/null | while read -r _ user _; do
    systemctl --user -M "$user@" stop qingjian-wayland.service >/dev/null 2>&1 || true
  done
fi
SCRIPT
chmod 755 "$wayland/DEBIAN/postinst" "$wayland/DEBIAN/prerm"

for dir in "$core" "$fcitx" "$wayland"; do
  dpkg-deb --root-owner-group -Zxz --build "$dir" "$stage/$(basename "$dir")_${version}_${arch}.deb" >/dev/null
done
ls "$stage"/*.deb
