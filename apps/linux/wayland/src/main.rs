//! 青简 Wayland 原生前端：自己当 input-method-v2 客户端，候选窗用青简渲染器自绘。
//!
//! 与 Fcitx5 插件、IBus 前端平级，连同一个 Server、说同一套协议：按键与焦点翻成 `LinuxEvent` 发给 Server，
//! 回包翻成 input-method-v2 的上屏与 preedit，帧画成位图贴到合成器给的 popup 上。设计见 `docs/design/linux-wayland.md`。
//!
//! `--stderr` 把日志打到终端（开发时用），缺省写 `$XDG_STATE_HOME/qingjian/logs/wayland.<日期>.log`；级别看 `RUST_LOG`。

#[cfg(target_os = "linux")]
mod error;
#[cfg(target_os = "linux")]
mod paths;
#[cfg(target_os = "linux")]
mod portal;
#[cfg(target_os = "linux")]
mod session;
#[cfg(target_os = "linux")]
mod view;
#[cfg(target_os = "linux")]
mod wayland;

fn main() {
    #[cfg(target_os = "linux")]
    std::process::exit(run());
    #[cfg(not(target_os = "linux"))]
    eprintln!("qingjian-linux-wayland 仅支持 Linux");
}

#[cfg(target_os = "linux")]
fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("qingjian-linux-wayland {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    let _guard = init_logging(args.iter().any(|arg| arg == "--stderr"));
    let desktop = portal::watch();
    let painter = view::Painter::new(paths::config_path(), Some(paths::themes_dir()), desktop);
    let socket = qingjian_platform::protocol::linux::socket_path();
    match wayland::run(painter, socket) {
        Ok(()) => 0,
        Err(error) => {
            tracing::error!(%error, "Wayland 前端退出");
            eprintln!("qingjian-linux-wayland: {error}");
            1
        }
    }
}

/// 日志：开发时打到终端，平时按天写文件（与 Server 同一个目录，反馈问题时整个目录打包）。
#[cfg(target_os = "linux")]
fn init_logging(stderr: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let file = (!stderr)
        .then(|| {
            tracing_appender::rolling::RollingFileAppender::builder()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("wayland")
                .filename_suffix("log")
                .max_log_files(7)
                .build(paths::log_dir())
                .inspect_err(|error| eprintln!("日志目录写不了，日志改打到终端：{error}"))
                .ok()
        })
        .flatten();
    match file {
        Some(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_ansi(false)
                .with_writer(writer)
                .init();
            Some(guard)
        }
        None => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(std::io::stderr)
                .init();
            None
        }
    }
}
