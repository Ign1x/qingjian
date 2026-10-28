//! zwp_input_method_v2 的事件：先进待生效的那一份，`done` 时整份生效，折成 [`Input`] 排队。

mod pending;

use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    ChangeCause, ContentHint, ContentPurpose,
};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::{
    self, ZwpInputMethodV2,
};

use pending::Pending;

use crate::session::{Field, Input, Surrounding};
use crate::wayland::state::Wayland;

/// 合成器那边关于输入框的状态。
#[derive(Debug, Default)]
pub struct Method {
    /// 下一次 `done` 生效的那一份。
    pending: Pending,

    /// 内容类型（密码框、敏感），`activate` 时回到缺省，之后一直有效到下次改。
    password: bool,

    sensitive: bool,

    /// 合成器认为我们激活着。
    active: bool,

    /// 最近一次 done 生效的状态，用来拒绝尚在 Server 往返中的旧选区与旧位置回包。
    field: Field,

    /// 收到过几次 `done`：commit 的 serial 必须等于它。
    pub dones: u32,

    /// 输入目标代次：焦点、隐私能力或应用主动修改的文字位置变化时递增。
    pub epoch: u64,
}

impl Method {
    pub fn selection_current(&self, field: &Field) -> bool {
        self.active
            && self.field.capabilities() == field.capabilities()
            && self.field.surrounding == field.surrounding
    }

    /// `done`：待生效的整份生效，返回要排进队的事。又激活一次（同一个应用里换了输入框）先当失活处理。
    fn done(&mut self) -> Vec<Input> {
        self.dones = self.dones.wrapping_add(1);
        let pending = std::mem::take(&mut self.pending);
        let field = Field {
            password: self.password,
            sensitive: self.sensitive,
            surrounding: pending.surrounding,
            external_change: pending.external_change,
        };
        let mut inputs = Vec::new();
        if pending.active.is_none() && self.active && field.invalidates(&self.field) {
            self.epoch += 1;
        }
        self.field = field.clone();
        match pending.active {
            Some(true) => {
                if self.active {
                    inputs.push(Input::Deactivate);
                    self.epoch += 1;
                }
                inputs.push(Input::Activate(field));
                self.active = true;
                self.epoch += 1;
            }
            Some(false) => {
                if self.active {
                    inputs.push(Input::Deactivate);
                    self.active = false;
                    self.epoch += 1;
                }
            }
            None if self.active => inputs.push(Input::Update(field)),
            None => {}
        }
        inputs
    }
}

impl Dispatch<ZwpInputMethodV2, ()> for Wayland {
    fn event(
        state: &mut Self,
        _: &ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let method = &mut state.method;
        match event {
            zwp_input_method_v2::Event::Activate => {
                // activate 把输入框那边的状态（前后文、内容类型、变化原因）全部回到初值
                method.pending = Pending {
                    active: Some(true),
                    ..Pending::default()
                };
                method.password = false;
                method.sensitive = false;
            }
            zwp_input_method_v2::Event::Deactivate => method.pending.active = Some(false),
            zwp_input_method_v2::Event::SurroundingText {
                text,
                cursor,
                anchor,
            } => {
                method.pending.surrounding = Some(Surrounding {
                    text,
                    cursor: cursor as usize,
                    anchor: anchor as usize,
                });
            }
            zwp_input_method_v2::Event::TextChangeCause { cause } => {
                method.pending.external_change = matches!(cause, WEnum::Value(ChangeCause::Other));
            }
            zwp_input_method_v2::Event::ContentType { hint, purpose } => {
                let hint = match hint {
                    WEnum::Value(hint) => hint,
                    WEnum::Unknown(bits) => ContentHint::from_bits_truncate(bits),
                };
                method.password = hint.contains(ContentHint::HiddenText)
                    || matches!(
                        purpose,
                        WEnum::Value(ContentPurpose::Password | ContentPurpose::Pin)
                    );
                method.sensitive = hint.contains(ContentHint::SensitiveData);
            }
            zwp_input_method_v2::Event::Done => {
                let inputs = method.done();
                state.inbox.extend(inputs);
            }
            zwp_input_method_v2::Event::Unavailable => state.unavailable = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
