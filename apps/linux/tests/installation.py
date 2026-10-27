#!/usr/bin/env python3
# //! 临时用户目录真实文件安装：重复安装、路径登记、校验拒绝与保留用户文件，以及 Wayland 原生版的前端与服务单元。
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

root = Path(__file__).resolve().parents[3]
helper = root / 'apps/linux/scripts/files.py'
with tempfile.TemporaryDirectory(prefix='qingjian-install-') as temporary:
    directory = Path(temporary)
    prefix = directory / 'prefix with spaces'
    data = directory / 'user data'
    server = directory / 'server'
    settings = directory / 'settings'
    plugin = directory / 'plugin'
    server.write_bytes(b'test binary\n')
    settings.write_bytes(b'test settings\n')
    plugin.write_bytes(b'test plugin\n')
    server.chmod(0o755)
    settings.chmod(0o755)
    env = dict(os.environ, XDG_DATA_HOME=str(data))
    args = ['python3', str(helper), 'install', str(prefix), '--root', str(root),
            '--server', str(server), '--settings', str(settings), '--plugin', str(plugin), '--sample']
    subprocess.run(args, env=env, check=True)
    subprocess.run(args, env=env, check=True)
    addon = data / 'fcitx5/addon/qingjian.conf'
    assert f'Library={prefix}/lib/fcitx5/qingjian\n' in addon.read_text()
    assert (prefix / 'bin/qingjian-linux-server').stat().st_mode & 0o111
    desktop = data / 'applications/qingjian-settings.desktop'
    assert f'Exec={prefix}/bin/qingjian-settings\n' in desktop.read_text()
    # hicolor 目录名必须与 PNG 实际像素一致：1024px 源图塞进 128x128 会让不缩放的托盘（Waybar）显示巨图（#547）。
    for size in (16, 22, 24, 32, 48, 64, 128, 256):
        icon = data / f'icons/hicolor/{size}x{size}/apps/qingjian.png'
        assert icon.is_file()
        header = icon.read_bytes()[:24]
        assert header[:8] == b'\x89PNG\r\n\x1a\n'
        width, height = struct.unpack('>II', header[16:24])
        assert (width, height) == (size, size), f'{icon} 实际尺寸 {width}x{height}'
    manifest = json.loads((prefix / 'share/qingjian/install-manifest.json').read_text())
    assert all(Path(filename).is_file() for filename in manifest)
    user = data / 'qingjian/user.tsv'
    user.parent.mkdir(parents=True, exist_ok=True)
    user.write_text('学习数据')
    other = data / 'fcitx5/addon/other.conf'
    other.write_text('其他输入法')
    modified = prefix / 'share/qingjian/resources/assets/sample/dict.tsv'
    modified.write_text('用户修改过的样例')
    failed = subprocess.run(args, env=env, capture_output=True)
    assert failed.returncode != 0
    subprocess.run(['python3', str(helper), 'uninstall', str(prefix)], env=env, check=True)
    subprocess.run(['python3', str(helper), 'uninstall', str(prefix)], env=env, check=True)
    assert not addon.exists() and not (prefix / 'bin/qingjian-linux-server').exists()
    assert not desktop.exists() and not (prefix / 'bin/qingjian-settings').exists()
    assert user.read_text() == '学习数据' and other.read_text() == '其他输入法'
    assert modified.read_text() == '用户修改过的样例'
    fcitx4_prefix = directory / 'fcitx4 prefix'
    config = directory / 'user config'
    fcitx4_env = dict(env, XDG_CONFIG_HOME=str(config))
    fcitx4_args = ['python3', str(helper), 'install', str(fcitx4_prefix), '--root', str(root),
                   '--server', str(server), '--settings', str(settings), '--plugin', str(plugin),
                   '--sample', '--flavor', 'fcitx4']
    subprocess.run(fcitx4_args, env=fcitx4_env, check=True)
    subprocess.run(fcitx4_args, env=fcitx4_env, check=True)
    fcitx4_addon = config / 'fcitx/addon/fcitx-qingjian.conf'
    assert f'Library={fcitx4_prefix}/lib/fcitx/fcitx-qingjian.so\n' in fcitx4_addon.read_text()
    assert (fcitx4_prefix / 'lib/fcitx/fcitx-qingjian.so').read_bytes() == plugin.read_bytes()
    imicon = config / 'fcitx/imicon/qingjian.png'
    tray_icon = data / 'icons/hicolor/48x48/apps/fcitx-qingjian.png'
    assert imicon.read_bytes() == tray_icon.read_bytes() == (root / 'assets/icon/fcitx4.png').read_bytes()
    subprocess.run(['python3', str(helper), 'uninstall', str(fcitx4_prefix)],
                   env=fcitx4_env, check=True)
    assert not fcitx4_addon.exists()
    assert not imicon.exists() and not tray_icon.exists()
    # Wayland 原生版：前端与它的 systemd 单元进清单；换成不带它的安装时清掉，卸载时也清掉，Server 的单元不受影响
    wayland_prefix = directory / 'wayland prefix'
    wayland_config = directory / 'wayland config'
    wayland = directory / 'wayland'
    wayland.write_bytes(b'test wayland\n')
    wayland.chmod(0o755)
    template = (root / 'apps/linux/scripts/qingjian-wayland.service.in').read_text()
    lines = template.splitlines()
    # 关键的几条：Server 跟着起、随 graphical-session.target 起停、退出码 1（起不来）不重启
    for line in ('Wants=qingjian-server.service', 'After=qingjian-server.service', 'PartOf=graphical-session.target',
                 'After=graphical-session.target', 'WantedBy=graphical-session.target', 'ExecStart=@EXEC@',
                 'Restart=on-failure', 'RestartPreventExitStatus=1'):
        assert line in lines, f'qingjian-wayland.service.in 缺 {line}'
    wayland_unit = directory / 'qingjian-wayland.service'
    wayland_unit.write_text(template.replace('@EXEC@', f'{wayland_prefix}/bin/qingjian-linux-wayland'))
    server_unit = directory / 'qingjian-server.service'
    server_unit.write_text((root / 'apps/linux/scripts/qingjian-server.service.in').read_text()
                           .replace('@EXEC@', f'{wayland_prefix}/bin/qingjian-linux-server'))
    wayland_env = dict(env, XDG_CONFIG_HOME=str(wayland_config))
    base_args = ['python3', str(helper), 'install', str(wayland_prefix), '--root', str(root),
                 '--server', str(server), '--settings', str(settings), '--sample', '--service', str(server_unit)]
    wayland_args = base_args + ['--wayland', str(wayland), '--wayland-service', str(wayland_unit)]
    subprocess.run(wayland_args, env=wayland_env, check=True)
    subprocess.run(wayland_args, env=wayland_env, check=True)
    installed_wayland = wayland_prefix / 'bin/qingjian-linux-wayland'
    installed_unit = wayland_config / 'systemd/user/qingjian-wayland.service'
    installed_server_unit = wayland_config / 'systemd/user/qingjian-server.service'
    assert installed_wayland.read_bytes() == wayland.read_bytes() and installed_wayland.stat().st_mode & 0o111
    assert f'ExecStart={installed_wayland}\n' in installed_unit.read_text()
    wayland_manifest = json.loads((wayland_prefix / 'share/qingjian/install-manifest.json').read_text())
    assert str(installed_wayland) in wayland_manifest and str(installed_unit) in wayland_manifest
    subprocess.run(base_args, env=wayland_env, check=True)
    assert not installed_wayland.exists() and not installed_unit.exists()
    assert installed_server_unit.is_file()
    subprocess.run(wayland_args, env=wayland_env, check=True)
    assert installed_wayland.is_file() and installed_unit.is_file()
    subprocess.run(['python3', str(helper), 'uninstall', str(wayland_prefix)], env=wayland_env, check=True)
    assert not installed_wayland.exists() and not installed_unit.exists() and not installed_server_unit.exists()
print('用户目录安装、重复安装、注册路径和卸载保留测试通过')
