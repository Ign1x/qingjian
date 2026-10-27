//! 桌面外观偏好的测试。`values` 不连 D-Bus；`bus` 对着私有总线上的假 portal 走真的往来，要 dbus-daemon，缺省不跑。

mod bus;
mod values;
