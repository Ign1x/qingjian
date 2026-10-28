#!/usr/bin/env bash
# //! 打预编译包 target/linux-package/qingjian-<版本>-linux-<cpu>.tar.gz（缺省，Fcitx5 版：Server、设置程序、Fcitx5 插件）
# //! 或 qingjian-wayland-<版本>-linux-<cpu>.tar.gz（--frontend wayland：Server、设置程序、Wayland 前端与两个单元模板），
# //! 两版都带产品数据与安装脚本。先跑 tools/release/data-fetch.sh；解开后的 install.sh 与源码安装共用 files.py 的清单与校验。
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
cd "$root"
frontend=fcitx5
while (($#)); do
  case "$1" in
    --frontend) frontend=${2:?--frontend 需要 fcitx5/wayland}; shift 2 ;;
    --help) echo '用法：package.sh [--frontend fcitx5|wayland]'; exit 0 ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done
case "$frontend" in
  fcitx5 | wayland) ;;
  *) echo "--frontend 只认 fcitx5 / wayland：$frontend" >&2; exit 2 ;;
esac
version=$(sed -n 's/^version = "\(.*\)"/\1/p' apps/linux/server/Cargo.toml | head -1)
if [[ "$version" == *-dev ]]; then
  version+="-$(git rev-parse --short=7 HEAD)"
  git diff --quiet HEAD || version+='+'
fi
tools/release/data-fetch.sh --verify
[[ -f data/models/hanzhang-tongbian/hanzhang-tongbian-small.qjm && -f data/generated/dict.qj ]] || { echo '缺少产品数据或含章模型，先运行 tools/release/data-fetch.sh' >&2; exit 1; }

cargo_output=$(python3 -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' "${CARGO_TARGET_DIR:-$root/target}")
cargo build --release --locked -p qingjian-linux-server -p qingjian-settings --target-dir "$cargo_output"
if [[ "$frontend" == wayland ]]; then
  # 单独一次 cargo：与设置程序一起编时 cargo 合并依赖的特性，别的包带进来的 winit 会把 wayland-backend 换成
  # dlopen libwayland 的那种；单独编是纯 Rust 的，运行时只 dlopen libxkbcommon（见 docs/notes/crate-notes.md 的 apps/linux/wayland 一节）
  cargo build --release --locked -p qingjian-linux-wayland --target-dir "$cargo_output"
else
  cmake_output="$root/target/fcitx5-package"
  cmake -S apps/linux/fcitx5 -B "$cmake_output" -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=OFF
  cmake --build "$cmake_output" --parallel "${CMAKE_BUILD_PARALLEL_LEVEL:-2}"
fi

if [[ "$frontend" == wayland ]]; then
  name="qingjian-wayland-$version-linux-$(uname -m)"
else
  name="qingjian-$version-linux-$(uname -m)"
fi
out="$root/target/linux-package"
stage="$out/$name"
rm -rf "$stage" "$out/$name.tar.gz"
mkdir -p "$stage/bin"
install -m 755 "$cargo_output/release/qingjian-linux-server" "$cargo_output/release/qingjian-settings" "$stage/bin/"
resources=(LICENSE apps/settings/linux assets/icon/linux/hicolor assets/stroke/LICENSE-CNS11643.txt assets/sample assets/glossary assets/levels assets/emoji assets/schemes \
  tools/release/data.lock target/release-data/qingjian-data.tar.gz)
if [[ "$frontend" == wayland ]]; then
  install -m 755 "$cargo_output/release/qingjian-linux-wayland" "$stage/bin/"
  install -m 644 apps/linux/scripts/qingjian-server.service.in apps/linux/scripts/qingjian-wayland.service.in "$stage/"
else
  mkdir -p "$stage/lib"
  install -m 755 "$cmake_output/qingjian.so" "$stage/lib/"
  resources+=(apps/linux/fcitx5/data)
fi
install -m 755 apps/linux/scripts/package-install.sh "$stage/install.sh"
if [[ "$frontend" == wayland ]]; then
  # 包里是哪个版本由这一行决定，装包的人不用再选（package-install.sh 的缺省是 fcitx5）
  sed -i 's/^frontend=fcitx5$/frontend=wayland/' "$stage/install.sh"
  grep -qx 'frontend=wayland' "$stage/install.sh" || { echo 'install.sh 里的版本行没换成 wayland' >&2; exit 1; }
fi
install -m 755 apps/linux/scripts/uninstall.sh "$stage/"
install -m 644 apps/linux/scripts/files.py "$stage/"
# 产品数据只带压缩包，install.sh 装前解开（files.py 按 data.lock 校验压缩包，再按包内摘要逐个校验解出的文件）
cp -r --parents "${resources[@]}" "$stage/"
tar -C "$out" -czf "$out/$name.tar.gz" "$name"
echo "已打包：$out/$name.tar.gz"
