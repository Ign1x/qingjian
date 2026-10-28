//! 横排矩阵（`[general] horizontal_grid`）：横排时 ↑ / ↓ 把一行候选展开成多行，视口与移动规则在 Core 的 [`Grid`]。
//! 只对握手报了 `features.grid` 的会话开（自己画候选窗的前端），框架面板画不了矩阵，那里 ↑ / ↓ 照旧。
//! 规则与 Windows Server 的同名方法一致，帧怎么排在 `Router::current_frame`。

use std::ops::Range;

use qingjian_core::candidate::arrows::Matrix;
use qingjian_core::{GRID_ROWS, Grid};

use super::Composed;
use crate::dispatch::Router;

impl Router {
    /// 这个会话认不认矩阵这套键：配置开着、横排，而且前端画得了。
    pub(in crate::dispatch) fn grid_keys(&self) -> bool {
        self.config.grid_keys()
            && self
                .focused
                .and_then(|session| self.sessions.get(&session))
                .is_some_and(|info| info.features.grid)
    }

    /// 给 `arrows::route` 的矩阵状态：没开、单行、展开后高亮在不在第一行。
    pub(in crate::dispatch) fn matrix(&self) -> Matrix {
        match self.grid {
            Some(_) => Matrix::Open {
                first_row: self.highlight < self.config.page_size,
            },
            None if self.grid_keys() => Matrix::Closed,
            None => Matrix::Off,
        }
    }

    /// 上 / 下键竖着移 `delta` 行（列不变，目标格没候选就取最近的）；还是单行就先从当前页展开（展开本身算变化）。
    /// 返回高亮是否真的动了。
    pub(in crate::dispatch) fn move_rows(&mut self, delta: isize) -> bool {
        let expanding = self.grid.is_none();
        self.shift_grid_rows(delta) || expanding
    }

    /// 矩阵里顶在第一排还往上：高亮回到第一个候选、视口滚回顶部。返回是否动了。
    pub(in crate::dispatch) fn jump_to_first(&mut self) -> bool {
        let Some(Composed::Candidates { layout, .. }) = &self.composed else {
            return false;
        };
        if self.grid.is_none() || self.highlight == 0 {
            return false;
        }
        self.highlight = 0;
        self.grid = Some(Grid::at(layout, 0));
        self.navigated = true;
        true
    }

    /// 矩阵里按阅读顺序移一格（越过行尾到下一行开头），跳过空位；没展开时不动。返回是否动了。
    pub(in crate::dispatch) fn move_cells(&mut self, delta: isize) -> bool {
        let Some(Composed::Candidates { layout, .. }) = &self.composed else {
            return false;
        };
        let Some(mut grid) = self.grid else {
            return false;
        };
        let moved = grid.move_cells(layout, self.highlight, delta);
        self.grid = Some(grid);
        self.land(moved)
    }

    /// 矩阵里整屏翻（翻页键）：一次 [`GRID_ROWS`] 行。返回是否动了。
    pub(in crate::dispatch) fn move_screens(&mut self, delta: isize) -> bool {
        self.shift_grid_rows(delta * GRID_ROWS as isize)
    }

    /// 收回单行（Esc 第一下），高亮留在原处。返回原来是不是展开着。
    pub(in crate::dispatch) fn collapse_grid(&mut self) -> bool {
        self.grid.take().is_some()
    }

    /// 热加载后这套键不生效了（开关关掉、改成竖排）：展开着的矩阵收回单行，挂起的会话也一样。
    pub(in crate::dispatch) fn collapse_disabled_grids(&mut self) {
        if !self.grid_keys() {
            self.grid = None;
        }
        let on = self.config.grid_keys();
        for info in self.sessions.values_mut() {
            if !(on && info.features.grid) {
                info.grid = None;
            }
        }
    }

    /// 这一帧的格子在整份候选里的下标：矩阵展开着是视口里那几行，否则是高亮所在的那一页。
    /// 前端点候选、问右键菜单报的是帧里的第几格，按它换回整份候选的下标。
    pub(in crate::dispatch) fn frame_cells(&self) -> Range<usize> {
        let page_size = self.config.page_size;
        if let (Some(grid), Some(Composed::Candidates { layout, .. })) =
            (&self.grid, &self.composed)
        {
            let rows = grid.rows(layout);
            return rows.start * page_size..rows.end * page_size;
        }
        let start = self.highlight / page_size * page_size;
        start..start + page_size
    }

    /// 矩阵视口里竖着移 `delta` 行；没展开就从当前页展开。返回高亮是否真的动了。
    fn shift_grid_rows(&mut self, delta: isize) -> bool {
        let Some(Composed::Candidates { layout, .. }) = &self.composed else {
            return false;
        };
        let page = self.highlight / self.config.page_size;
        let mut grid = self.grid.take().unwrap_or_else(|| Grid::at(layout, page));
        let moved = grid.move_rows(layout, self.highlight, delta);
        self.grid = Some(grid);
        self.land(moved)
    }

    /// 高亮落到 `index`（`None` 是没动）。返回是否动了。
    fn land(&mut self, index: Option<usize>) -> bool {
        let Some(index) = index else {
            return false;
        };
        self.highlight = index;
        self.navigated = true;
        true
    }
}
