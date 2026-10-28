//! 被测进程在调试日志里记的「候选窗上能点的地方」（`src/view/canvas/mod.rs` 的 `Canvas::describe`）：
//! `candidate0@62,44 item1@78,80`，各自中心的 surface 坐标（逻辑像素）。鼠标测试按它找要点的位置。

use std::collections::HashMap;

use super::Log;

/// 那行日志的消息（`src/wayland/present.rs` 的 `paint`）。
const MARK: &str = "候选窗上能点的地方";

/// 一张候选窗上能点的东西：名字（`candidate0`、`item1`）→ 中心。
#[derive(Debug, Clone)]
pub struct Targets(HashMap<String, (f64, f64)>);

impl Targets {
    /// 等日志第 `from` 行起记下的那一行，读出来。
    pub fn wait(lines: &Log<String>, from: usize) -> Self {
        let line = lines.wait(from, "候选窗上能点的地方", |line| {
            line.contains(MARK)
        });
        Self(
            line.split_whitespace()
                .filter_map(|token| {
                    let token = token.rsplit('=').next()?;
                    let (name, at) = token.split_once('@')?;
                    let (x, y) = at.split_once(',')?;
                    Some((name.to_owned(), (x.parse().ok()?, y.parse().ok()?)))
                })
                .collect(),
        )
    }

    /// `name` 的中心；没有就带着整份 panic。
    pub fn at(&self, name: &str) -> (f64, f64) {
        *self
            .0
            .get(name)
            .unwrap_or_else(|| panic!("候选窗上没有 {name}：{:?}", self.0))
    }

    pub fn has(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
}
