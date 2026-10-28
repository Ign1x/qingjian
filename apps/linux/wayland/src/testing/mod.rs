//! 测试共用的东西：私有的 D-Bus 总线（portal 与托盘的 D-Bus 往来都对着它测，不碰会话总线）。

mod bus;

pub use bus::Bus;
