# 青简的 RPM spec，出三个包：qingjian（Server、IBus 前端、词库、systemd 用户服务）、qingjian-fcitx5（插件）
# 与 qingjian-wayland（Wayland 原生版的前端与它的 systemd 用户服务）。
#
# **装的是预编译好的文件**：rpm/assemble.sh 先把所有东西摆进 %%{qj_root}，这里只拷进 buildroot，所以没有 %%build。
# 从源码构建要把整棵 cargo 依赖树弄进来，自建分发不值得；真要进发行版官方仓库时再写 %%build 与 cargo 打包宏。
# 动态库依赖不手写：rpmbuild 扫 ELF 的 NEEDED 自动生成（libc.so.6(GLIBC_2.34)(64bit)、libFcitx5Core.so.7()(64bit) 这些）；
# 例外是 Wayland 前端运行时 dlopen 的 libxkbcommon，NEEDED 里没有，在它的子包里手写。
# 版本号由 build.sh 从 apps/linux/server/Cargo.toml 与 git 短哈希算好传进来；CHANGELOG 由维护者统一写，这里不带 %%changelog。

# 文件是外面编好、剥好的，别让 rpm 再剥符号 / 造 build-id 链接 / 出 debuginfo
%global __os_install_post %{nil}
%global _build_id_links none
%global debug_package %{nil}
# 没有 %%changelog（见上），别拿它定时间戳
%global source_date_epoch_from_changelog 0

Name:           qingjian
Version:        %{qj_version}
Release:        %{qj_release}%{?dist}
Summary:        青简输入法：Server、IBus 前端与词库
License:        GPL-3.0-or-later
URL:            https://qingjian.app
Recommends:     (ibus or qingjian-fcitx5 or qingjian-wayland)

%description
跨平台拼音输入法。本包是引擎（Server，systemd 用户服务）、IBus 前端与随包词库；
用 fcitx5 的再装 qingjian-fcitx5，Hyprland、sway、niri 这类桌面可改装 qingjian-wayland。
Server 装完就起；IBus 要重新登录一次（或 ibus restart）才看得到青简。

%package fcitx5
Summary:        青简输入法的 fcitx5 插件
Requires:       %{name}%{?_isa} = %{version}-%{release}
Requires:       fcitx5
# Fedora 上没有它，登录时 fcitx5 不自己起、GNOME 仍走 IBus；别的发行版没有这个包时弱依赖直接忽略
Recommends:     fcitx5-autostart

%description fcitx5
让 fcitx5 能用青简；引擎在 qingjian 包里。装完重启 fcitx5，在配置工具里添加「青简」。

%package wayland
Summary:        青简输入法 Wayland 原生版
Requires:       %{name}%{?_isa} = %{version}-%{release}
# 运行时按这个 soname dlopen，自动依赖看不到；写 soname 而不写包名，Fedora（libxkbcommon）与 openSUSE（libxkbcommon0）都认。
# 只打 x86_64，所以直接写 (64bit)
Requires:       libxkbcommon.so.0()(64bit)
# 字体只关系候选窗画得好不好；名字是 Fedora 的，别的发行版没有这几个包时弱依赖直接忽略
Recommends:     google-noto-sans-cjk-vf-fonts
Recommends:     google-noto-color-emoji-fonts
Recommends:     fontconfig

%description wayland
不经过 Fcitx / IBus，青简自己当输入法，候选窗口自绘，主题、字体与字号都生效；
用于 Hyprland、sway（1.10 起）、niri、labwc 等桌面，KDE 与 GNOME 用 qingjian-fcitx5 或 IBus。
装完不会自己启用：要用的用户执行 systemctl --user enable --now qingjian-wayland.service，
并关掉 fcitx5（或它的 Wayland 输入法前端），同一时间只能有一个输入法。

%install
cp -a %{qj_root}/. %{buildroot}/

# Server 挂到每个用户的会话里自启。--global 只对之后起来的用户 systemd 实例生效，而注销再登录得快时
# 实例根本不重起；所以已经登录的用户直接在他们正在跑的实例里重载并（重）启动（与 deb 的 postinst 一致）
%post
systemctl --global enable qingjian-server.service >/dev/null 2>&1 || :
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
  systemctl --user -M "$user@" daemon-reload >/dev/null 2>&1 || :
  systemctl --user -M "$user@" restart qingjian-server.service >/dev/null 2>&1 || :
done

%preun
if [ $1 -eq 0 ]; then
  systemctl --global disable qingjian-server.service >/dev/null 2>&1 || :
  loginctl list-users --no-legend 2>/dev/null | while read -r _ user _; do
    systemctl --user -M "$user@" stop qingjian-server.service >/dev/null 2>&1 || :
  done
fi

# 不 --global enable：同一个 seat 只能有一个输入法，GNOME / KDE 上它也起不来，要用的用户自己 enable。
# 已经登录、正开着它的用户换上新的（try-restart 只重启在跑的）；第一次装时说一声怎么启用（与 deb 的 postinst 一致）
%post wayland
loginctl list-users --no-legend 2>/dev/null | while read -r _ user _; do
  systemctl --user -M "$user@" daemon-reload >/dev/null 2>&1 || :
  systemctl --user -M "$user@" try-restart qingjian-wayland.service >/dev/null 2>&1 || :
done
if [ $1 -eq 1 ]; then
  # 拆成短行：dnf5 按终端宽度截断装包脚本的输出
  echo "青简 Wayland 原生版：要用的用户执行" >&2
  echo "  systemctl --user enable --now qingjian-wayland.service" >&2
  echo "  并关掉 fcitx5（或它的 Wayland 输入法前端）" >&2
fi

%preun wayland
if [ $1 -eq 0 ]; then
  systemctl --global disable qingjian-wayland.service >/dev/null 2>&1 || :
  loginctl list-users --no-legend 2>/dev/null | while read -r _ user _; do
    systemctl --user -M "$user@" stop qingjian-wayland.service >/dev/null 2>&1 || :
  done
fi

%files
%license /usr/share/licenses/qingjian/LICENSE
/usr/bin/qingjian-linux-server
/usr/bin/qingjian-linux-ibus
/usr/share/qingjian
/usr/share/ibus/component/qingjian.xml
/usr/lib/systemd/user/qingjian-server.service
/usr/share/icons/hicolor/*/apps/qingjian.png

%files fcitx5
%{_libdir}/fcitx5/qingjian.so
/usr/share/fcitx5/addon/qingjian.conf
/usr/share/fcitx5/inputmethod/qingjian.conf

%files wayland
/usr/bin/qingjian-linux-wayland
/usr/lib/systemd/user/qingjian-wayland.service
