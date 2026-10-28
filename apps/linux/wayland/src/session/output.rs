//! [`super::Session`] 对合成器那一侧要做的事。真实现在 `crate::wayland`，测试里换成记录器。

use qingjian_platform::protocol::Frame;
use qingjian_platform::protocol::linux::InputMode;

use super::{Field, Modifiers, Preedit};

pub trait Output {
    /// 输入目标代次：焦点、隐私能力或应用主动修改的文字位置变化时递增。
    fn epoch(&self) -> u64;

    /// 不阻塞地读一遍合成器新到的事件（新的排进队尾、代次随之更新）。
    /// 等 Server 回包期间输入目标可能已变化，回包落地前先调它再比代次。
    fn sync(&mut self);

    /// 最近一次 done 生效的选区与隐私能力仍与请求发起时相同。
    /// sync 只把事件排队，Session 尚未处理这些事件，读取选区前须查合成器侧的最新状态。
    fn selection_current(&self, field: &Field) -> bool;

    /// 接管或交还键盘。
    fn grab(&mut self, on: bool);

    /// 虚拟键盘换键位表。必须先于虚拟键盘上的任何按键与修饰键。
    fn keymap(&mut self, keymap: &str);

    /// 经虚拟键盘把一个键原样转给应用。
    fn forward(&mut self, code: u32, pressed: bool, time: u32);

    /// 经虚拟键盘同步修饰键。
    fn modifiers(&mut self, modifiers: Modifiers);

    /// 一次原子的文字更新：先上屏 `commit`（可以是空串），再把行内拼音换成 `preedit`。
    fn text(&mut self, commit: &str, preedit: &Preedit);

    /// 画出这一帧并贴到候选窗上，返回真正画出来的译词义项 `(候选在本页的位置, 义项编号)`。
    fn show(&mut self, frame: &Frame) -> Vec<(usize, usize)>;

    /// 收起候选窗。
    fn hide(&mut self);

    /// 在候选窗上弹出右键菜单，`items` 是各项的字，摆在最近一次按下鼠标的地方；画不出来返回 `false`。
    /// 换上新的一帧（[`Self::show`]）或收起候选窗时菜单一并收起。
    fn open_menu(&mut self, items: &[String]) -> bool;

    /// 收起右键菜单，候选窗照旧。
    fn close_menu(&mut self);

    /// 输入模式变了（`None` 是不知道了：与 Server 断开）。之后画的候选窗按它显示主题里的 `mode.*`。
    fn mode(&mut self, mode: Option<&InputMode>);

    /// 在候选窗的位置画出中 / 英提示（换掉候选窗上的东西，[`Self::hide`] 收起）；画不出来（没有字体）时返回 `false`。
    fn hint(&mut self, mode: &InputMode) -> bool;
}
