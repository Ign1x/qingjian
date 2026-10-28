//! 键位提示画哪一种键盘。

use qingjian_core::shuangpin::KeyHint;
use serde::{Deserialize, Serialize};

use super::RootChartId;

/// 双拼键位与五笔字根图二选一（混输时画字根图）。线上是 `{"Shuangpin": {…}}` 或 `{"Roots": "wubi86"}`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyHintContent {
    /// 双拼键位：原样是 Core 算的那一份，标着敲了声母之后哪些键能接成音节。
    Shuangpin(KeyHint),

    /// 五笔字根图：字形是编进 Core 的静态数据，线上只带是哪一张。
    Roots(RootChartId),
}
