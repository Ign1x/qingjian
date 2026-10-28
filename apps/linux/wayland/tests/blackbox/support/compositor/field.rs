//! 应用那边输入框的状态：text-input-v3 报给合成器的前后文、变化原因与内容类型。
//! 合成器每次 done 之前把它原样转给输入法（wlroots 的 `relay_send_im_state` 就是这样，每次都发）。

use wayland_protocols::wp::text_input::zv3::server::zwp_text_input_v3::{
    ChangeCause, ContentHint, ContentPurpose,
};
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_v2::ZwpInputMethodV2;

#[derive(Debug, Clone, Default)]
pub struct Field {
    /// 光标附近的文字，与光标、选区另一端的 UTF-8 字节偏移；`None` 是应用不报前后文（foot 这类）。
    pub surrounding: Option<(String, u32, u32)>,

    /// 内容类型；`None` 是应用不报（之前报过的输入法照旧算数）。
    pub content_type: Option<(ContentHint, ContentPurpose)>,

    /// 这次前后文变化是应用自己改的（text_change_cause 为 other），不是输入法上屏引起的。
    pub changed_by_app: bool,
}

impl Field {
    /// 一段文字，光标在 `cursor`、选区另一端在 `anchor`（字节偏移，两者相等是没有选区）。
    pub fn text(text: &str, cursor: usize, anchor: usize) -> Self {
        let offset = |bytes: usize| u32::try_from(bytes).expect("偏移放得进 u32");
        Self {
            surrounding: Some((text.to_owned(), offset(cursor), offset(anchor))),
            ..Self::default()
        }
    }

    /// 密码框：purpose 是 password，hint 带隐藏文字与敏感（GTK4 的密码框就这样报）。
    pub fn password() -> Self {
        Self {
            content_type: Some((
                ContentHint::HiddenText | ContentHint::SensitiveData,
                ContentPurpose::Password,
            )),
            ..Self::default()
        }
    }

    /// 普通文字框，明确报了内容类型（密码框点了「显示密码」之后就是这样）。
    pub fn normal() -> Self {
        Self {
            content_type: Some((ContentHint::None, ContentPurpose::Normal)),
            ..Self::default()
        }
    }

    /// 同一段文字，标成敏感内容（hint sensitive_data）：照常组句，但不该带前后文。
    pub fn sensitive(self) -> Self {
        Self {
            content_type: Some((ContentHint::SensitiveData, ContentPurpose::Normal)),
            ..self
        }
    }

    /// 同一个状态，这次的变化是应用自己改的。
    pub fn by_app(self) -> Self {
        Self {
            changed_by_app: true,
            ..self
        }
    }

    /// 发给输入法：前后文、变化原因、内容类型，done 由调用方发。
    pub fn send(&self, method: &ZwpInputMethodV2) {
        if let Some((text, cursor, anchor)) = &self.surrounding {
            method.surrounding_text(text.clone(), *cursor, *anchor);
        }
        method.text_change_cause(if self.changed_by_app {
            ChangeCause::Other
        } else {
            ChangeCause::InputMethod
        });
        if let Some((hint, purpose)) = self.content_type {
            method.content_type(hint, purpose);
        }
    }
}
