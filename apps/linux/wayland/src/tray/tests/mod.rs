//! 托盘的测试。`menu` 与 `icons` 不连 D-Bus；`bus` 对着私有总线上的假托盘宿主走真的往来，要 dbus-daemon，缺省不跑。

mod bus;
mod handle;
mod icons;
mod menu;
