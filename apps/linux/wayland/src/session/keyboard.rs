//! 键位表、修饰键、按键与重复：哪个键交给 Server、哪个原样转给应用。

use std::time::Instant;

use qingjian_platform::protocol::KeyOutcome;
use qingjian_platform::protocol::linux::key::to_key_event;

use super::keys::Held;
use super::{Key, Modifiers, Output, Server, Session};

impl<S: Server> Session<S> {
    pub(super) fn keymap(&mut self, keymap: String, out: &mut impl Output) {
        // 每次重新 grab 合成器都会再发一遍同样的键位表，按内容比较，变了才转给虚拟键盘
        if self.keymap.as_deref() != Some(keymap.as_str()) {
            out.keymap(&keymap);
            self.keymap = Some(keymap);
        }
    }

    pub(super) fn modifiers(&mut self, modifiers: Modifiers, out: &mut impl Output) {
        // 虚拟键盘收到键位表之前发别的是协议错误，合成器会断开整条连接
        if self.keymap.is_some() {
            out.modifiers(modifiers);
        }
    }

    pub(super) fn key(&mut self, key: Key, out: &mut impl Output, now: Instant) {
        if self.keymap.is_none() {
            return;
        }
        if key.pressed {
            self.press(key, out, now);
        } else {
            self.release(key, out, now);
        }
    }

    fn press(&mut self, key: Key, out: &mut impl Output, now: Instant) {
        // 后按下的键接替前一个的重复
        self.repeat.stop();
        if !self.grabbed {
            // 交还键盘之后才排到的按下：转出去的话它的抬起不再经过这里，虚拟键盘上就卡住了
            return;
        }
        if !self.serving() {
            self.flush(out, now);
            out.forward(key.code, true, key.time);
            self.keys.press(
                key.code,
                Held {
                    forwarded: true,
                    reported: false,
                },
            );
            return;
        }
        let event = to_key_event(key.keysym, key.mask);
        match self.server.key(event, false, self.surrounding()) {
            Ok(reply) => {
                let forwarded = reply.outcome == KeyOutcome::Passthrough;
                // 快捷键冲洗时回包带着原文：先上屏，再把键交给应用
                self.apply(reply, key.epoch, out, now);
                if forwarded {
                    // 快捷键冲洗上屏的原文必须先到应用，攒着的这时也要发
                    self.flush(out, now);
                    out.forward(key.code, true, key.time);
                } else {
                    self.repeat.start(key, now);
                }
                self.keys.press(
                    key.code,
                    Held {
                        forwarded,
                        reported: true,
                    },
                );
            }
            Err(error) => {
                self.lost(&error, out);
                self.flush(out, now);
                out.forward(key.code, true, key.time);
                self.keys.press(
                    key.code,
                    Held {
                        forwarded: true,
                        reported: false,
                    },
                );
            }
        }
    }

    fn release(&mut self, key: Key, out: &mut impl Output, now: Instant) {
        if self.repeat.holds(key.code) {
            self.repeat.stop();
        }
        let held = self.keys.release(key.code);
        if held.is_some_and(|held| held.reported) && self.serving() {
            let event = to_key_event(key.keysym, key.mask);
            match self.server.key(event, true, None) {
                // 单击 Shift 切中英时回包可能带着原文：先上屏，再转抬键
                Ok(reply) => self.apply(reply, key.epoch, out, now),
                Err(error) => self.lost(&error, out),
            }
        }
        // 抬起跟着按下走，不管 Server 对抬起怎么说；没见过按下的（grab 之前就按着的）也转出去
        if held.is_none_or(|held| held.forwarded) {
            out.forward(key.code, false, key.time);
        }
    }

    /// 被吃掉的键按住不放：自己再报一次按下。组句删空了 Server 改回放行，就转一次按下给应用，
    /// 之后的重复交给应用自己（它收到了按下、没收到抬起）。
    pub(super) fn repeat_key(&mut self, key: Key, out: &mut impl Output, now: Instant) {
        if !self.serving() || !self.keys.holds(key.code) {
            self.repeat.stop();
            return;
        }
        let event = to_key_event(key.keysym, key.mask);
        match self.server.key(event, false, self.surrounding()) {
            Ok(reply) => {
                let forwarded = reply.outcome == KeyOutcome::Passthrough;
                self.apply(reply, key.epoch, out, now);
                if forwarded {
                    self.flush(out, now);
                    out.forward(key.code, true, self.repeat.time(now));
                    self.keys.forward(key.code);
                    self.repeat.stop();
                } else {
                    self.repeat.schedule(now);
                }
            }
            Err(error) => {
                self.lost(&error, out);
                self.repeat.stop();
            }
        }
    }
}
