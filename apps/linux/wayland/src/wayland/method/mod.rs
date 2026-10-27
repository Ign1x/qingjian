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

use super::Wayland;
use crate::session::{Field, Input, Surrounding};

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

    /// 收到过几次 `done`：commit 的 serial 必须等于它。
    pub dones: u32,

    /// 焦点代次：激活、失活各加一。
    pub epoch: u64,
}

impl Method {
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
mod tests {
    use super::*;

    fn done(method: &mut Method) -> Vec<Input> {
        method.done()
    }

    /// 激活、更新、换输入框、失活：代次只在进出时变，`done` 一次计一次。
    #[test]
    fn transitions_follow_done() {
        let mut method = Method::default();
        method.pending.active = Some(true);
        assert_eq!(done(&mut method), [Input::Activate(Field::default())]);
        assert_eq!((method.epoch, method.dones), (1, 1));

        method.pending.external_change = true;
        assert_eq!(
            done(&mut method),
            [Input::Update(Field {
                external_change: true,
                ..Field::default()
            })]
        );
        assert_eq!(method.epoch, 1, "更新不换代次");

        method.pending.active = Some(true);
        assert_eq!(
            done(&mut method),
            [Input::Deactivate, Input::Activate(Field::default())]
        );
        assert_eq!(method.epoch, 3);

        method.pending.active = Some(false);
        assert_eq!(done(&mut method), [Input::Deactivate]);
        assert!(done(&mut method).is_empty(), "失活着不报更新");
        assert_eq!((method.epoch, method.dones), (4, 5));
    }

    /// 内容类型一直有效到下次改；前后文与变化原因每次 `done` 之后回到初值。
    #[test]
    fn content_type_persists_but_surrounding_does_not() {
        let mut method = Method {
            password: true,
            ..Method::default()
        };
        method.pending.active = Some(true);
        method.pending.surrounding = Some(Surrounding {
            text: "a".to_owned(),
            cursor: 1,
            anchor: 1,
        });
        let inputs = done(&mut method);
        assert!(
            matches!(&inputs[..], [Input::Activate(field)] if field.password && field.surrounding.is_some())
        );
        let inputs = done(&mut method);
        assert!(
            matches!(&inputs[..], [Input::Update(field)] if field.password && field.surrounding.is_none())
        );
    }
}
