//! 输入模式：Server 附在回包上的当前模式，与托盘 / 状态显示面上切换它的命令。

mod command;
mod state;

pub use command::ModeCommand;
pub use state::InputMode;
