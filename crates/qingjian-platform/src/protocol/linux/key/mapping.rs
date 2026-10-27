//! keysym + 修饰键掩码 → 协议里的 [`KeyEvent`]。
//!
//! **这张表必须与 Fcitx5 插件的 `apps/linux/fcitx5/src/key/mapping.cpp` 一字不差**：
//! 两个前端喂的是同一个 Server、同一套 `[shortcut]` 配置，映射不一致的话同一个键在两个框架下
//! 行为就不一样（最容易出问题的是翻页键与 Shift+数字）。改这里要同时改那边。
//!
//! 虚拟键码用 Windows 那套值（`0x08` 退格、`0x25` 左…），来源见
//! [`crate::protocol::KeyEvent`]：Server 的按键分流是照它写的，Linux 这边照抄。

use super::{keysym, modifiers};
use crate::protocol::KeyEvent;

/// Shift + 数字行敲出来的符号，按数字 0–9 排。
///
/// 译词与删候选的快捷键按**物理数字行**认（`Shift+2` 是第 2 条），而 X11 给的 keysym 是 `@`。
/// 所以这里把它折回 `0x32`，`character` 仍保留 `@` 不动——上屏的还是符号本身。
const SHIFTED_DIGITS: [char; 10] = [')', '!', '@', '#', '$', '%', '^', '&', '*', '('];

/// 把一次按键（keysym + X11 修饰键掩码）翻成 [`KeyEvent`]。
///
/// `state` 是 X11 的修饰键掩码（IBus 的 `IBusModifierType` 与它同值，Wayland 前端按 xkb 的修饰键状态拼）。
/// `english_mode` 由调用方按当前中 / 英状态补，这一层不知道输入法的模式。
pub fn to_key_event(keysym_value: u32, state: u32) -> KeyEvent {
    let mut modifiers = modifiers::from_mask(state);
    // ISO_Left_Tab 是「Shift+Tab」那一个 keysym，掩码里不一定带 Shift 位，补上
    if keysym_value == keysym::ISO_LEFT_TAB {
        modifiers.shift = true;
    }
    KeyEvent {
        virtual_key: virtual_key(keysym_value, modifiers.shift),
        character: keysym::to_char(keysym_value),
        modifiers,
    }
}

/// keysym → 虚拟键码。认不出来的原样交出去（ASCII 字母与符号就走这一条）。
fn virtual_key(keysym_value: u32, shift: bool) -> u32 {
    // Shift+数字先判：keysym 是符号，要的却是数字行的键码
    if keysym_value < 128
        && shift
        && let Some(character) = char::from_u32(keysym_value)
        && let Some(digit) = SHIFTED_DIGITS.iter().position(|c| *c == character)
    {
        return 0x30 + digit as u32;
    }
    if (keysym::KP_0..=keysym::KP_9).contains(&keysym_value) {
        return 0x60 + (keysym_value - keysym::KP_0);
    }
    match keysym_value {
        keysym::BACKSPACE => 0x08,
        keysym::TAB | keysym::ISO_LEFT_TAB => 0x09,
        keysym::RETURN | keysym::KP_ENTER => 0x0d,
        keysym::KP_BEGIN => 0x0c,
        keysym::ESCAPE => 0x1b,
        keysym::SHIFT_L | keysym::SHIFT_R => 0x10,
        keysym::CONTROL_L | keysym::CONTROL_R => 0x11,
        keysym::PAGE_UP | keysym::KP_PAGE_UP => 0x21,
        keysym::PAGE_DOWN | keysym::KP_PAGE_DOWN => 0x22,
        keysym::END | keysym::KP_END => 0x23,
        keysym::HOME | keysym::KP_HOME => 0x24,
        keysym::LEFT | keysym::KP_LEFT => 0x25,
        keysym::UP | keysym::KP_UP => 0x26,
        keysym::RIGHT | keysym::KP_RIGHT => 0x27,
        keysym::DOWN | keysym::KP_DOWN => 0x28,
        keysym::KP_INSERT => 0x2d,
        keysym::DELETE | keysym::KP_DELETE => 0x2e,
        keysym::KP_MULTIPLY => 0x6a,
        keysym::KP_ADD => 0x6b,
        keysym::KP_SEPARATOR => 0x6c,
        keysym::KP_SUBTRACT => 0x6d,
        keysym::KP_DECIMAL => 0x6e,
        keysym::KP_DIVIDE => 0x6f,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// X11 的 Shift 位。
    const SHIFT: u32 = 1 << 0;

    /// 字母原样过去：keysym 就是键码，字符跟着。
    #[test]
    fn letters_pass_through() {
        let event = to_key_event('n' as u32, 0);
        assert_eq!(event.virtual_key, 'n' as u32);
        assert_eq!(event.character, Some('n'));
        assert!(!event.modifiers.shift);
    }

    /// Shift+2：键码折回数字行的 `0x32`，字符仍是 `@`（这是 Linux 上真机踩过的坑）。
    #[test]
    fn shifted_digits_fold_back_to_the_number_row() {
        let event = to_key_event('@' as u32, SHIFT);
        assert_eq!(event.virtual_key, 0x32);
        assert_eq!(event.character, Some('@'));
        assert!(event.modifiers.shift);
    }

    /// 没按 Shift 的 `@`（有的布局上直接就有）不折。
    #[test]
    fn unshifted_symbol_is_not_folded() {
        assert_eq!(to_key_event('@' as u32, 0).virtual_key, '@' as u32);
    }

    /// 功能键与小键盘按 Windows 那套键码走。
    #[test]
    fn function_and_keypad_keys_use_windows_codes() {
        assert_eq!(to_key_event(keysym::BACKSPACE, 0).virtual_key, 0x08);
        assert_eq!(to_key_event(keysym::LEFT, 0).virtual_key, 0x25);
        assert_eq!(to_key_event(keysym::KP_LEFT, 0).virtual_key, 0x25);
        assert_eq!(to_key_event(keysym::KP_ENTER, 0).virtual_key, 0x0d);
        assert_eq!(to_key_event(keysym::KP_0, 0).virtual_key, 0x60);
        assert_eq!(to_key_event(keysym::KP_9, 0).virtual_key, 0x69);
        assert_eq!(to_key_event(keysym::SHIFT_L, 0).virtual_key, 0x10);
        // 小键盘数字有字符，方向键没有
        assert_eq!(to_key_event(keysym::KP_0, 0).character, Some('0'));
        assert_eq!(to_key_event(keysym::LEFT, 0).character, None);
    }

    /// Shift+Tab 是另一个 keysym，掩码里没 Shift 也要认成带 Shift 的 Tab。
    #[test]
    fn iso_left_tab_is_shift_tab() {
        let event = to_key_event(keysym::ISO_LEFT_TAB, 0);
        assert_eq!(event.virtual_key, 0x09);
        assert!(event.modifiers.shift);
    }

    /// Ctrl 键本身走 `0x11`：单击 Ctrl 切中 / 英（`[shortcut] switch_mode`）靠它认。
    #[test]
    fn control_keys_use_the_control_code() {
        assert_eq!(to_key_event(keysym::CONTROL_L, 0).virtual_key, 0x11);
        assert_eq!(to_key_event(keysym::CONTROL_R, 0).virtual_key, 0x11);
    }

    /// Super（Mod4）与 Hyper（Mod3）都算 `win`，与 Fcitx5 插件一样。
    #[test]
    fn super_and_hyper_both_count_as_win() {
        assert!(to_key_event('a' as u32, 1 << 6).modifiers.win);
        assert!(to_key_event('a' as u32, 1 << 5).modifiers.win);
        assert!(
            !to_key_event('a' as u32, 1 << 4).modifiers.win,
            "Mod2 是 NumLock"
        );
    }

    /// 字符按 keysym 查 Unicode：小键盘的运算符也有字符，控制字符没有。
    #[test]
    fn characters_follow_keysym_to_unicode() {
        assert_eq!(to_key_event(keysym::KP_ADD, 0).character, Some('+'));
        assert_eq!(to_key_event(keysym::KP_DECIMAL, 0).character, Some('.'));
        assert_eq!(to_key_event(xkeysym::key::KP_Space, 0).character, Some(' '));
        assert_eq!(to_key_event(xkeysym::key::KP_Equal, 0).character, Some('='));
        // Latin-2 段的 Ą 与 Unicode 段的 中
        assert_eq!(to_key_event(0x1a1, 0).character, Some('Ą'));
        assert_eq!(to_key_event(0x0100_4e2d, 0).character, Some('中'));
        for control in [
            keysym::BACKSPACE,
            keysym::RETURN,
            keysym::ESCAPE,
            keysym::DELETE,
            keysym::TAB,
        ] {
            assert_eq!(to_key_event(control, 0).character, None, "{control:#x}");
        }
    }

    /// 与 Fcitx5 插件的 `mapping.cpp` 逐条对照：那边的每个 `case FcitxKey_X: code = 0x..` 这里都要给出同一个键码，
    /// Shift+数字的那串符号也要一样。那边加了新键而这里没跟上，这条就挂。
    #[test]
    fn table_matches_the_fcitx5_plugin() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../apps/linux/fcitx5/src/key/mapping.cpp"
        );
        let source = std::fs::read_to_string(path).expect("读 Fcitx5 插件的键表");
        let mut checked = 0;
        for line in source
            .lines()
            .filter(|line| line.contains("code = 0x") && line.contains("case "))
        {
            let code_at = line.find("code = 0x").expect("有键码");
            let hex = &line[code_at + "code = 0x".len()..];
            let hex = &hex[..hex.find(';').expect("键码以分号结尾")];
            let code = u32::from_str_radix(hex, 16).expect("十六进制键码");
            for rest in line[..code_at].split("FcitxKey_").skip(1) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                let value = fcitx_keysym(&name)
                    .unwrap_or_else(|| panic!("对照表里没有 FcitxKey_{name}，补进 fcitx_keysym"));
                assert_eq!(to_key_event(value, 0).virtual_key, code, "FcitxKey_{name}");
                checked += 1;
            }
        }
        assert!(
            checked >= 30,
            "只对上了 {checked} 个键，多半是 mapping.cpp 的写法变了"
        );
        let shifted = source
            .split("shiftedDigits = \"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("Shift+数字那串符号");
        assert_eq!(shifted.chars().collect::<Vec<_>>(), SHIFTED_DIGITS);
        let win = source
            .lines()
            .find(|line| line.contains("\"win\""))
            .expect("win 那一行");
        assert!(
            win.contains("Super") && win.contains("Hyper"),
            "win 的认法变了：{win}"
        );
    }

    /// `mapping.cpp` 里出现的 Fcitx keysym 名 → 值（与 X11 同名同值）。
    fn fcitx_keysym(name: &str) -> Option<u32> {
        use xkeysym::key;
        Some(match name {
            "BackSpace" => key::BackSpace,
            "Tab" => key::Tab,
            "ISO_Left_Tab" => key::ISO_Left_Tab,
            "Return" => key::Return,
            "KP_Enter" => key::KP_Enter,
            "Escape" => key::Escape,
            "Page_Up" => key::Page_Up,
            "KP_Page_Up" => key::KP_Page_Up,
            "Page_Down" => key::Page_Down,
            "KP_Page_Down" => key::KP_Page_Down,
            "End" => key::End,
            "KP_End" => key::KP_End,
            "Home" => key::Home,
            "KP_Home" => key::KP_Home,
            "Left" => key::Left,
            "KP_Left" => key::KP_Left,
            "Up" => key::Up,
            "KP_Up" => key::KP_Up,
            "Right" => key::Right,
            "KP_Right" => key::KP_Right,
            "Down" => key::Down,
            "KP_Down" => key::KP_Down,
            "Delete" => key::Delete,
            "KP_Delete" => key::KP_Delete,
            "KP_Insert" => key::KP_Insert,
            "KP_Begin" => key::KP_Begin,
            "Shift_L" => key::Shift_L,
            "Shift_R" => key::Shift_R,
            "Control_L" => key::Control_L,
            "Control_R" => key::Control_R,
            "KP_Multiply" => key::KP_Multiply,
            "KP_Add" => key::KP_Add,
            "KP_Separator" => key::KP_Separator,
            "KP_Subtract" => key::KP_Subtract,
            "KP_Decimal" => key::KP_Decimal,
            "KP_Divide" => key::KP_Divide,
            _ => return None,
        })
    }
}
