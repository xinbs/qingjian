//! 云猜词从第一页第二格开始，本地首选保留，其余本地候选顺延。
//! 不预留、不画占位；自定义短语固定位置不移动，云端词排在固定格之后。
//! 平台收到晚到的结果时检查插入点，用户已选中会被移动的候选或翻页时不插入。

mod candidate_layout;
mod cell;
mod grid;

pub use candidate_layout::CandidateLayout;
pub use cell::Cell;
pub use grid::{GRID_ROWS, Grid, MAX_CELL_EMS};
