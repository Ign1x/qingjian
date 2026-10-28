//! 字根图的标识：字形随 Core 编进各个进程，线上只说是哪一张。

use qingjian_core::root_chart::RootChart;
use serde::{Deserialize, Serialize};

/// 线上是小写的名字（`"wubi86"`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RootChartId {
    /// 86 版五笔。
    Wubi86,
}

impl RootChartId {
    /// Core 给的这张字根图是哪一张；认不出的（Core 加了新的、这里还没跟上）为 `None`。
    pub fn of(chart: &RootChart) -> Option<Self> {
        // Core 的字根图都是 `&'static` 单例，按地址认，不逐个字形比较
        std::ptr::eq(chart, RootChart::wubi86()).then_some(Self::Wubi86)
    }

    /// 前端按标识从 Core 取字形。
    pub fn chart(self) -> &'static RootChart {
        match self {
            Self::Wubi86 => RootChart::wubi86(),
        }
    }
}
