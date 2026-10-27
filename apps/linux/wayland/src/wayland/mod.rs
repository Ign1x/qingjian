//! Wayland 一侧：连合成器、绑定全局对象、主循环。合成器的事件折成 [`Input`](crate::session::Input) 交给会话，
//! 会话的决定经 [`Context`] 变成 input-method-v2、虚拟键盘与候选窗上的请求。

mod context;
mod grab;
mod keyboard;
mod memfd;
mod method;
mod noop;
mod popup;
mod present;
mod shm;
mod state;
mod toplevel;
mod xkb;

use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use wayland_client::backend::WaylandError;
use wayland_client::globals::registry_queue_init;
use wayland_client::{Connection, EventQueue};

use context::Context;
use state::Wayland;
use xkb::Xkb;

use crate::error::Error;
use crate::session::{Link, Session};
use crate::view::Painter;

/// 收到 SIGINT / SIGTERM：主循环转完这一圈就退出。
static STOP: AtomicBool = AtomicBool::new(false);

/// 跑到收到退出信号或连接出错为止。
pub fn run(painter: Option<Painter>, socket: PathBuf) -> Result<(), Error> {
    if !Xkb::available() {
        return Err(Error::Xkb);
    }
    let connection = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Wayland>(&connection)?;
    let mut wayland = Wayland::new(&globals, &queue.handle(), painter)?;
    queue.roundtrip(&mut wayland)?;
    if wayland.unavailable {
        return Err(Error::Unavailable);
    }
    install_signal_handlers();
    tracing::info!(server = %socket.display(), "输入法就绪");
    let mut session = Session::new(Link::new(socket));
    while !STOP.load(Ordering::Relaxed) {
        queue.dispatch_pending(&mut wayland)?;
        if wayland.unavailable {
            return Err(Error::Unavailable);
        }
        while let Some(input) = wayland.inbox.pop_front() {
            let mut context = Context {
                wayland: &mut wayland,
                queue: &mut queue,
            };
            session.handle(input, &mut context, Instant::now());
        }
        let mut context = Context {
            wayland: &mut wayland,
            queue: &mut queue,
        };
        session.tick(&mut context, Instant::now());
        wayland.tick(Instant::now());
        queue.flush()?;
        let deadline = [session.deadline(), wayland.deadline()]
            .into_iter()
            .flatten()
            .min();
        wait(&queue, deadline)?;
    }
    tracing::info!("收到退出信号");
    let mut context = Context {
        wayland: &mut wayland,
        queue: &mut queue,
    };
    session.shutdown(&mut context);
    queue.flush()?;
    Ok(())
}

/// 等合成器的事件或下一个到点的事，读进队列。被信号打断就直接回去看 [`STOP`]。
fn wait(queue: &EventQueue<Wayland>, deadline: Option<Instant>) -> Result<(), Error> {
    let Some(guard) = queue.prepare_read() else {
        // 队列里已经有没处理的事件
        return Ok(());
    };
    let timeout = deadline.map_or(-1, |deadline| {
        let left = deadline.saturating_duration_since(Instant::now());
        // 向上取整到毫秒，不然离到点不足 1 毫秒时会空转
        left.as_micros().div_ceil(1000).min(i32::MAX as u128) as i32
    });
    let mut fds = [libc::pollfd {
        fd: guard.connection_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];
    // SAFETY: fds 是长度为 1 的本地数组
    let ready = unsafe { libc::poll(fds.as_mut_ptr(), 1, timeout) };
    if ready > 0 {
        match guard.read() {
            Ok(_) => {}
            Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn install_signal_handlers() {
    extern "C" fn stop(_: libc::c_int) {
        STOP.store(true, Ordering::Relaxed);
    }
    // SAFETY: 处理函数只写一个原子量
    unsafe {
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
    }
}
