//! xkbcommon（运行时 dlopen）：按 grab 给的键位表与修饰键状态，把键码算成 keysym 与 X11 修饰键掩码。

use std::ffi::{CStr, CString};

use xkbcommon_dl::{
    XkbCommon, xkb_context, xkb_context_flags, xkb_keymap, xkb_keymap_compile_flags,
    xkb_keymap_format, xkb_state, xkb_state_component, xkbcommon_option,
};

/// X11 的八个真实修饰键，按掩码里的位序。
const REAL_MODIFIERS: [&CStr; 8] = [
    c"Shift", c"Lock", c"Control", c"Mod1", c"Mod2", c"Mod3", c"Mod4", c"Mod5",
];

/// 一份键位表与它上面的修饰键状态。
pub struct Xkb {
    lib: &'static XkbCommon,

    context: *mut xkb_context,

    keymap: *mut xkb_keymap,

    state: *mut xkb_state,

    /// [`REAL_MODIFIERS`] 在这份键位表里的下标；键位表里没有的是 `XKB_MOD_INVALID`。
    modifiers: [u32; 8],
}

impl Xkb {
    /// libxkbcommon 能不能加载：加载不了就没法解读按键，不能接管键盘。
    pub fn available() -> bool {
        xkbcommon_option().is_some()
    }

    /// 编译一份 xkb 文本键位表；坏了返回 `None`。
    pub fn new(text: &str) -> Option<Self> {
        let lib = xkbcommon_option()?;
        let source = CString::new(text).ok()?;
        // SAFETY: 都是 libxkbcommon 的构造函数，空指针表示失败；失败时把已经建好的释放掉
        unsafe {
            let context = (lib.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS);
            if context.is_null() {
                return None;
            }
            let keymap = (lib.xkb_keymap_new_from_string)(
                context,
                source.as_ptr(),
                xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            );
            if keymap.is_null() {
                (lib.xkb_context_unref)(context);
                return None;
            }
            let state = (lib.xkb_state_new)(keymap);
            if state.is_null() {
                (lib.xkb_keymap_unref)(keymap);
                (lib.xkb_context_unref)(context);
                return None;
            }
            let modifiers =
                REAL_MODIFIERS.map(|name| (lib.xkb_keymap_mod_get_index)(keymap, name.as_ptr()));
            Some(Self {
                lib,
                context,
                keymap,
                state,
                modifiers,
            })
        }
    }

    /// 合成器发来的修饰键状态（wl_keyboard 的 group 是生效的布局组，放在 locked 那一格）。
    pub fn update(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        // SAFETY: state 在 self 活着时一直有效
        unsafe {
            (self.lib.xkb_state_update_mask)(self.state, depressed, latched, locked, 0, 0, group);
        }
    }

    /// evdev 键码按当前状态算出的 keysym（xkb 键码是 evdev 加 8）；出好几个的算 0。
    pub fn keysym(&self, code: u32) -> u32 {
        // SAFETY: 同上
        unsafe { (self.lib.xkb_state_key_get_one_sym)(self.state, code + 8) }
    }

    /// 按住会不会重复（修饰键不重复）。
    pub fn repeats(&self, code: u32) -> bool {
        // SAFETY: 同上
        unsafe { (self.lib.xkb_keymap_key_repeats)(self.keymap, code + 8) == 1 }
    }

    /// 当前生效的 X11 修饰键掩码：Shift 第 0 位、Lock 第 1 位、Control 第 2 位、Mod1–Mod5 第 3–7 位。
    pub fn mask(&self) -> u32 {
        self.modifiers
            .iter()
            .enumerate()
            .filter(|(_, index)| {
                // SAFETY: 同上；不存在的下标 libxkbcommon 返回 -1，不是 1
                unsafe {
                    (self.lib.xkb_state_mod_index_is_active)(
                        self.state,
                        **index,
                        xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
                    ) == 1
                }
            })
            .fold(0, |mask, (bit, _)| mask | 1 << bit)
    }
}

impl Drop for Xkb {
    fn drop(&mut self) {
        // SAFETY: 三个指针都是 new 里建的、只在这里释放一次
        unsafe {
            (self.lib.xkb_state_unref)(self.state);
            (self.lib.xkb_keymap_unref)(self.keymap);
            (self.lib.xkb_context_unref)(self.context);
        }
    }
}
