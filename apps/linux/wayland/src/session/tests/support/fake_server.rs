//! 会话测试的假 Server。

use super::{Answer, Call, identity};
use crate::session::Server;
use qingjian_platform::protocol::linux::client::{ClientError, Reply};
use qingjian_platform::protocol::linux::{AppSurrounding, Capabilities, DisplayIdentity, MenuItem};
use qingjian_platform::protocol::{Frame, KeyEvent, KeyOutcome};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 照脚本回话的 Server：按下按 `presses` 依次答；抬起与轮询照真 Server 的样子回当前帧。
#[derive(Default)]
pub struct FakeServer {
    pub calls: Vec<Call>,

    pub presses: VecDeque<Answer>,

    /// 抬起的特别答复（单击 Shift 切换）；没有就回放行加当前帧。
    pub releases: VecDeque<Answer>,

    /// 下一次轮询要给的新帧（重排结果到了）。
    pub polled: Option<Frame>,

    /// Server 挂了：所有请求都失败。
    pub down: bool,

    /// 下一个报了能读选区的按下回「要选区」（选区翻译快捷键），带这个请求编号；那一键的答复等回了选区再按 `presses` 给。
    pub selection_request: Option<u64>,

    /// 回包上的 `translation_pending`：选区翻译在等译文。
    pub pending: bool,

    /// 点候选、翻页、点菜单项的答复，按先后用掉；没有就吃掉、帧不变。
    pub clicks: VecDeque<Answer>,

    /// 右键菜单的几项。
    pub menu: Vec<MenuItem>,

    /// 选着的应用。
    app: Option<String>,

    frame: Frame,

    revision: u64,

    connected: bool,

    retry_at: Option<Instant>,
}

impl FakeServer {
    fn reply(
        &mut self,
        outcome: KeyOutcome,
        commit: Option<String>,
        english: Option<bool>,
    ) -> Reply {
        self.revision += 1;
        Reply {
            outcome,
            commit,
            frame: self.frame.clone(),
            identity: Some(identity(self.revision)),
            english,
            translation_pending: self.pending,
            ..Reply::default()
        }
    }

    fn check(&mut self) -> Result<(), ClientError> {
        if self.down {
            self.connected = false;
            self.retry_at = Some(Instant::now() + Duration::from_secs(2));
            return Err(ClientError::Closed);
        }
        self.connected = true;
        Ok(())
    }

    /// 帧在 Server 那边换了、前端还不知道：之后带旧身份的点选都过期。
    pub fn advance(&mut self) {
        self.revision += 1;
    }

    /// 点选的身份对不上当前帧就像真 Server 那样回 `Ignored`，对得上照脚本答。
    fn click(&mut self, identity: &DisplayIdentity) -> Result<Reply, ClientError> {
        self.check()?;
        if identity.revision != self.revision {
            return Ok(Reply::default());
        }
        let answer = self.clicks.pop_front();
        let (commit, english) = match answer {
            Some(answer) => {
                self.frame = answer.frame;
                self.pending = false;
                (answer.commit, answer.english)
            }
            None => (None, None),
        };
        Ok(self.reply(KeyOutcome::Consumed, commit, english))
    }
}

impl Server for FakeServer {
    fn select(&mut self, app: Option<&str>) {
        if self.app.as_deref() != app {
            self.app = app.map(str::to_owned);
            self.calls.push(Call::Select(self.app.clone()));
        }
    }

    fn app(&self) -> Option<&str> {
        self.app.as_deref()
    }

    fn capabilities(&mut self, capabilities: Capabilities) -> Result<(), ClientError> {
        self.calls.push(Call::Capabilities(capabilities));
        Ok(())
    }

    fn focus(&mut self, focused: bool) -> Result<Reply, ClientError> {
        self.calls.push(Call::Focus(focused));
        self.check()?;
        if !focused {
            self.frame = Frame::default();
            self.pending = false;
        }
        Ok(self.reply(KeyOutcome::Passthrough, None, None))
    }

    fn deactivate(&mut self) -> Result<Reply, ClientError> {
        self.calls.push(Call::Deactivate);
        self.check()?;
        self.frame = Frame::default();
        self.pending = false;
        Ok(self.reply(KeyOutcome::Passthrough, None, None))
    }

    fn reset(&mut self) -> Result<Reply, ClientError> {
        self.calls.push(Call::Reset);
        self.check()?;
        self.frame = Frame::default();
        self.pending = false;
        Ok(self.reply(KeyOutcome::Passthrough, None, None))
    }

    fn key(
        &mut self,
        event: KeyEvent,
        release: bool,
        surrounding: Option<AppSurrounding>,
        selection_supported: bool,
    ) -> Result<Reply, ClientError> {
        self.calls.push(Call::Key {
            code: event.virtual_key,
            release,
            context: surrounding.is_some(),
            selection: selection_supported,
        });
        self.check()?;
        if selection_supported && let Some(request) = self.selection_request.take() {
            return Ok(Reply {
                selection_request: Some(request),
                ..Reply::default()
            });
        }
        let script = if release {
            &mut self.releases
        } else {
            &mut self.presses
        };
        match script.pop_front() {
            Some(answer) => {
                self.frame = answer.frame;
                Ok(self.reply(answer.outcome, answer.commit, answer.english))
            }
            None => Ok(self.reply(KeyOutcome::Passthrough, None, None)),
        }
    }

    fn selection(&mut self, request: u64, text: String) -> Result<Reply, ClientError> {
        self.calls.push(Call::Selection(request, text));
        self.check()?;
        match self.presses.pop_front() {
            Some(answer) => {
                self.frame = answer.frame;
                Ok(self.reply(answer.outcome, answer.commit, answer.english))
            }
            None => Ok(self.reply(KeyOutcome::Passthrough, None, None)),
        }
    }

    fn poll(&mut self) -> Result<Reply, ClientError> {
        self.calls.push(Call::Poll);
        self.check()?;
        let mut reply = match self.polled.take() {
            Some(frame) => {
                self.frame = frame;
                self.reply(KeyOutcome::Passthrough, None, None)
            }
            // 帧没变：版本号沿用
            None => {
                let mut reply = self.reply(KeyOutcome::Passthrough, None, None);
                self.revision -= 1;
                reply.identity = Some(identity(self.revision));
                reply
            }
        };
        reply.outcome = KeyOutcome::Passthrough;
        Ok(reply)
    }

    fn acknowledge(
        &mut self,
        identity: DisplayIdentity,
        _senses: Vec<(usize, usize)>,
    ) -> Result<(), ClientError> {
        self.calls.push(Call::Acknowledge(identity.revision));
        Ok(())
    }

    fn connected(&self) -> bool {
        self.connected
    }

    fn retry_at(&self) -> Option<Instant> {
        self.retry_at
    }

    fn candidate(&mut self, identity: DisplayIdentity, index: usize) -> Result<Reply, ClientError> {
        self.calls.push(Call::Candidate(index, identity.revision));
        self.click(&identity)
    }

    fn page(&mut self, identity: DisplayIdentity, next: bool) -> Result<Reply, ClientError> {
        self.calls.push(Call::Page(next));
        self.click(&identity)
    }

    /// 只是问一句：帧与身份都不变；身份过期回空的。
    fn menu(
        &mut self,
        identity: DisplayIdentity,
        index: usize,
    ) -> Result<Vec<MenuItem>, ClientError> {
        self.calls.push(Call::Menu(index));
        self.check()?;
        if identity.revision != self.revision {
            return Ok(Vec::new());
        }
        Ok(self.menu.clone())
    }

    fn menu_pick(
        &mut self,
        identity: DisplayIdentity,
        index: usize,
        action: u32,
    ) -> Result<Reply, ClientError> {
        self.calls.push(Call::MenuPick(index, action));
        self.click(&identity)
    }
}
