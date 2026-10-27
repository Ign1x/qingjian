//! 假合成器：用 wayland-server 搭的最小合成器，在测试进程里自己的线程上听沙箱里的一个私有 socket，
//! 被测进程当它是真合成器连上来。提供 wl_compositor、wl_shm、wl_seat、input-method-v2、虚拟键盘、
//! viewporter 与分数缩放；客户端的请求按先后记进 [`Compositor::requests`]，测试经 [`Compositor::call`] 让它发事件。

mod buffer;
mod keyboard;
mod method;
mod peer;
mod picture;
mod pool;
mod request;
mod scale;
mod setup;
mod state;
mod surface;
mod wayland;

pub use picture::Picture;
pub use request::Request;
pub use setup::Setup;
pub use state::State;

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use wayland_protocols::wp::fractional_scale::v1::server::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::viewporter::server::wp_viewporter::WpViewporter;
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_server::protocol::wl_compositor::WlCompositor;
use wayland_server::protocol::wl_seat::WlSeat;
use wayland_server::protocol::wl_shm::WlShm;
use wayland_server::{Display, ListeningSocket};

use buffer::Buffer;
use peer::Peer;
use pool::Pool;
use surface::Surface;

use crate::support::{KEYMAP, Log};

/// socket 的名字（在沙箱这个 XDG_RUNTIME_DIR 里），被测进程的 WAYLAND_DISPLAY。
pub const DISPLAY: &str = "wayland-qingjian-test";

/// 在合成器线程上跑的一件事。
type Command = Box<dyn FnOnce(&mut State) + Send>;

pub struct Compositor {
    /// 客户端发来的请求，按先后。
    pub requests: Log<Request>,

    commands: Sender<Command>,

    /// 往这里写一个字节叫醒合成器线程（它睡在 poll 上）。
    waker: UnixStream,

    stop: Arc<AtomicBool>,

    thread: Option<JoinHandle<()>>,
}

impl Compositor {
    /// 在 `dir` 里开 socket，起合成器线程。键位表也写进 `dir`：grab 发它要一个 fd。
    pub fn start(dir: &Path, setup: Setup) -> Self {
        let listener = ListeningSocket::bind_absolute(dir.join(DISPLAY)).expect("合成器的 socket");
        let keymap_path = dir.join("keymap.xkb");
        // 合成器发的键位表连同结尾的 0，size 也算上它
        let mut keymap = KEYMAP.as_bytes().to_vec();
        keymap.push(0);
        std::fs::write(&keymap_path, &keymap).expect("写键位表");
        let keymap_file = File::open(&keymap_path).expect("打开键位表");
        let keymap_size = u32::try_from(keymap.len()).expect("键位表的大小");
        let requests = Log::default();
        let (commands, inbox) = mpsc::channel();
        let (waker, wakee) = UnixStream::pair().expect("叫醒合成器线程用的 socket");
        wakee.set_nonblocking(true).expect("改成非阻塞");
        let stop = Arc::new(AtomicBool::new(false));
        let state = State::new(requests.clone(), setup, keymap_file, keymap_size);
        let thread = thread::spawn({
            let stop = stop.clone();
            move || serve(listener, state, &inbox, &wakee, &stop)
        });
        Self {
            requests,
            commands,
            waker,
            stop,
            thread: Some(thread),
        }
    }

    /// 在合成器线程上做一件事（发事件、读状态），等它做完、取回结果。事件随后由合成器线程送出去。
    pub fn call<R: Send + 'static>(
        &self,
        action: impl FnOnce(&mut State) -> R + Send + 'static,
    ) -> R {
        let (reply, result) = mpsc::sync_channel(1);
        self.commands
            .send(Box::new(move |state| {
                let _ = reply.send(action(state));
            }))
            .expect("合成器线程已经没了");
        self.wake();
        result.recv().expect("合成器线程已经没了")
    }

    /// activate + done，等于应用里点进了一个输入框。
    pub fn activate(&self) {
        assert!(self.call(State::activate), "被测进程还没拿输入法对象");
    }

    pub fn deactivate(&self) {
        assert!(self.call(State::deactivate), "被测进程还没拿输入法对象");
    }

    pub fn done(&self) {
        assert!(self.call(State::done), "被测进程还没拿输入法对象");
    }

    /// 发过几次 done。
    pub fn dones(&self) -> u32 {
        self.call(|state| state.dones)
    }

    /// 经 grab 按下一个键（evdev 键码），返回时间戳。
    pub fn press(&self, code: u32) -> u32 {
        self.call(move |state| state.key(code, true))
            .expect("被测进程没有接管键盘")
    }

    pub fn release(&self, code: u32) -> u32 {
        self.call(move |state| state.key(code, false))
            .expect("被测进程没有接管键盘")
    }

    /// 按下立刻抬起：两个事件一起发，中间等多久都不会撞上被测进程自己的按键重复（600 毫秒后开始）。
    pub fn tap(&self, code: u32) {
        let sent = self.call(move |state| state.key(code, true).and(state.key(code, false)));
        sent.expect("被测进程没有接管键盘");
    }

    /// 同步点：把修饰键原样再发一次，等被测进程经虚拟键盘转出来。那时它已经处理完前面的事件，
    /// 由它们发出的请求也都到了。键盘不在它手里（或在这期间交还了）就不等：交还了的 grab 上的事件它收不到。
    pub fn sync(&self) {
        let mark = self.requests.len();
        if self.call(State::resend_modifiers) {
            self.requests.wait(mark, "转出来的修饰键", |request| {
                matches!(request, Request::Modifiers { .. } | Request::Release)
            });
        }
    }

    fn wake(&self) {
        let _ = (&self.waker).write(&[1]);
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.wake();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// 合成器线程：等客户端的请求、新连接或测试的命令，处理完把事件送出去，直到收工。
fn serve(
    listener: ListeningSocket,
    mut state: State,
    inbox: &Receiver<Command>,
    wakee: &UnixStream,
    stop: &AtomicBool,
) {
    let mut display = Display::<State>::new().expect("建合成器");
    let mut handle = display.handle();
    handle.create_global::<State, WlCompositor, ()>(6, ());
    handle.create_global::<State, WlShm, ()>(1, ());
    handle.create_global::<State, WlSeat, ()>(7, ());
    if state.setup.input_method {
        handle.create_global::<State, ZwpInputMethodManagerV2, ()>(1, ());
    }
    handle.create_global::<State, ZwpVirtualKeyboardManagerV1, ()>(1, ());
    handle.create_global::<State, WpViewporter, ()>(1, ());
    handle.create_global::<State, WpFractionalScaleManagerV1, ()>(1, ());
    let clients = display.backend().poll_fd().as_raw_fd();
    loop {
        let mut fds = [listener.as_raw_fd(), clients, wakee.as_raw_fd()].map(|fd| libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        });
        // SAFETY: fds 是本地数组，长度与传的一致
        unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        let mut drained = [0; 64];
        while matches!((&*wakee).read(&mut drained), Ok(read) if read > 0) {}
        if stop.load(Ordering::Relaxed) {
            return;
        }
        while let Ok(Some(stream)) = listener.accept() {
            handle
                .insert_client(stream, Arc::new(Peer::new(state.requests.clone())))
                .expect("接上客户端");
        }
        let _ = display.dispatch_clients(&mut state);
        while let Ok(command) = inbox.try_recv() {
            command(&mut state);
        }
        let _ = display.flush_clients();
    }
}
