//! 本地候选与云猜词的分页排布；保留首选和自定义短语固定位置。

use super::Cell;
use crate::candidate::{Candidate, CandidateKind};

/// 本地候选 + 云端词的分页排布。云端词从第二格开始，固定位置短语存在时顺延。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CandidateLayout {
    /// 本地候选，顺序就是 Engine 排好的顺序。
    local: Vec<Candidate>,

    /// 已到的云端词（≤ 槽位数，与本地候选去过重）。
    cloud: Vec<Candidate>,

    /// 每页几格。
    page_size: usize,

    /// 配置的云端槽位数；0 表示不要云端词。
    slots: usize,
}

impl CandidateLayout {
    pub fn new(local: Vec<Candidate>, page_size: usize, slots: usize) -> Self {
        Self {
            local,
            cloud: Vec::new(),
            page_size: page_size.max(1),
            slots,
        }
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn local(&self) -> &[Candidate] {
        &self.local
    }

    pub fn cloud(&self) -> &[Candidate] {
        &self.cloud
    }

    /// 引擎补译文时遍历真实候选，保留格子位置与分页配置。
    pub(crate) fn candidates_mut(&mut self) -> impl Iterator<Item = &mut Candidate> {
        self.local.iter_mut().chain(self.cloud.iter_mut())
    }

    /// 第一页最多给云端几格：本地候选至少占第一格；没有本地候选（问字模式）时整页都给云端。
    pub fn capacity(&self) -> usize {
        if self.local.is_empty() {
            self.page_size
        } else {
            let fixed = self
                .local
                .iter()
                .filter_map(|c| match c.kind {
                    CandidateKind::Custom(n) => Some(n),
                    _ => None,
                })
                .max()
                .unwrap_or(1);
            self.slots.min(self.page_size.saturating_sub(fixed.max(1)))
        }
    }

    /// 云端词的插入位置；自定义短语占固定格，云端词放在最后一个固定格之后。
    pub fn cloud_insertion_index(&self) -> usize {
        self.local
            .iter()
            .filter_map(|candidate| match candidate.kind {
                CandidateKind::Custom(position) => Some(position),
                _ => None,
            })
            .max()
            .unwrap_or(1)
            .max(1)
            .min(self.local_len())
    }

    /// 高亮项位于插入点之前时才允许补云端词，避免把用户正在选择的词顶走。
    pub fn can_insert_cloud(&self, highlighted: usize) -> bool {
        self.local.is_empty() || highlighted < self.cloud_insertion_index()
    }

    /// 云端词到了：去重后从第二格开始填入，多出来的丢掉。返回填进去的条数。
    pub fn set_cloud(&mut self, words: Vec<Candidate>) -> usize {
        self.cloud.clear();
        let capacity = self.capacity();
        for mut word in words {
            if self.cloud.len() >= capacity
                || self.local.iter().any(|c| c.text == word.text)
                || self.cloud.iter().any(|c| c.text == word.text)
            {
                continue;
            }
            word.kind = CandidateKind::Cloud;
            self.cloud.push(word);
        }
        self.cloud.len()
    }

    /// 本地格数包含最大固定位置之前的空格。
    fn local_len(&self) -> usize {
        self.local
            .iter()
            .filter_map(|c| match c.kind {
                CandidateKind::Custom(position) => Some(position),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            .max(self.local.len())
    }

    /// 真实本地候选按固定位置放置，其余候选依次填入空格。
    fn local_cells(&self) -> Vec<Cell<'_>> {
        let mut cells = vec![Cell::Empty; self.local_len()];
        for candidate in &self.local {
            if let CandidateKind::Custom(position) = candidate.kind
                && let Some(cell) = position.checked_sub(1).and_then(|i| cells.get_mut(i))
            {
                *cell = Cell::Local(candidate);
            }
        }
        let mut normal = self
            .local
            .iter()
            .filter(|c| !matches!(c.kind, CandidateKind::Custom(_)));
        for cell in &mut cells {
            if matches!(cell, Cell::Empty)
                && let Some(candidate) = normal.next()
            {
                *cell = Cell::Local(candidate);
            }
        }
        cells
    }

    /// 全部格子按索引顺序排开；空位只属于布局。
    pub fn cells(&self) -> Vec<Cell<'_>> {
        let mut cells = self.local_cells();
        let first = self.cloud_insertion_index();
        cells.splice(first..first, self.cloud.iter().map(Cell::Cloud));
        cells
    }

    pub fn len(&self) -> usize {
        self.local_len() + self.cloud.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn pages(&self) -> usize {
        self.len().div_ceil(self.page_size)
    }

    /// 第 `index` 格的候选；越界返回 `None`。
    pub fn candidate(&self, index: usize) -> Option<&Candidate> {
        self.cells().get(index).and_then(|cell| cell.candidate())
    }

    /// 第 `page` 页的格子。
    pub fn page(&self, page: usize) -> Vec<Cell<'_>> {
        self.cells()
            .into_iter()
            .skip(page * self.page_size)
            .take(self.page_size)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(text: &str) -> Candidate {
        Candidate {
            text: text.into(),
            kind: CandidateKind::Chinese,
            syllables: vec!["zhang".into(), "tao".into()],
            reading: None,
            translation: None,
            aux_code: None,
        }
    }

    fn cloud(text: &str) -> Candidate {
        Candidate {
            kind: CandidateKind::Cloud,
            ..local(text)
        }
    }

    fn texts(cells: &[Cell<'_>]) -> Vec<String> {
        cells
            .iter()
            .map(|cell| match cell {
                Cell::Local(c) => c.text.clone(),
                Cell::Cloud(c) => format!("☁{}", c.text),
                Cell::Empty => "<empty>".into(),
            })
            .collect()
    }

    fn many(count: usize) -> Vec<Candidate> {
        (0..count).map(|i| local(&format!("本{i}"))).collect()
    }

    #[test]
    fn cloud_words_start_second_and_preserve_the_first_local_candidate() {
        let mut layout = CandidateLayout::new(many(12), 9, 3);
        assert_eq!(
            texts(&layout.page(0)),
            [
                "本0", "本1", "本2", "本3", "本4", "本5", "本6", "本7", "本8"
            ]
        );
        assert_eq!(layout.pages(), 2);

        // 四条云端词只取前三条；本地首选不动，其余本地候选顺延。
        let filled = layout.set_cloud(vec![cloud("云0"), cloud("云1"), cloud("云2"), cloud("云3")]);
        assert_eq!(filled, 3);
        assert_eq!(
            texts(&layout.page(0)),
            [
                "本0", "☁云0", "☁云1", "☁云2", "本1", "本2", "本3", "本4", "本5"
            ]
        );
        assert_eq!(
            texts(&layout.page(1)),
            ["本6", "本7", "本8", "本9", "本10", "本11"]
        );
        assert_eq!(layout.candidate(1).unwrap().text, "云0");
        assert_eq!(layout.candidate(9).unwrap().text, "本6");
    }

    #[test]
    fn duplicates_of_local_candidates_are_dropped_and_fewer_words_take_fewer_cells() {
        let mut layout = CandidateLayout::new(many(4), 9, 2);
        assert_eq!(layout.set_cloud(vec![cloud("本2"), cloud("云0")]), 1);
        assert_eq!(texts(&layout.page(0)), ["本0", "☁云0", "本1", "本2", "本3"]);
        assert_eq!(layout.local()[2].kind, CandidateKind::Chinese);
        assert_eq!(layout.pages(), 1);
    }

    #[test]
    fn no_words_means_nothing_changes() {
        let mut layout = CandidateLayout::new(many(12), 9, 2);
        assert_eq!(layout.set_cloud(Vec::new()), 0);
        assert_eq!(layout.page(0).len(), 9);
        assert_eq!(texts(&layout.page(1)), ["本9", "本10", "本11"]);
    }

    #[test]
    fn zero_slots_means_no_cloud_cells_at_all() {
        let mut layout = CandidateLayout::new(many(3), 9, 0);
        assert_eq!(layout.set_cloud(vec![cloud("云0")]), 0);
        assert_eq!(texts(&layout.page(0)), ["本0", "本1", "本2"]);
    }

    #[test]
    fn without_local_candidates_the_whole_page_goes_to_the_cloud() {
        let mut layout = CandidateLayout::new(Vec::new(), 9, 2);
        assert_eq!(
            layout.set_cloud(vec![cloud("森"), cloud("淼"), cloud("焱")]),
            3
        );
        assert_eq!(texts(&layout.page(0)), ["☁森", "☁淼", "☁焱"]);
        assert_eq!(layout.candidate(0).unwrap().text, "森");
    }

    #[test]
    fn slots_never_push_the_first_local_candidate_off_the_first_page() {
        let mut layout = CandidateLayout::new(many(5), 3, 5);
        assert_eq!(layout.capacity(), 2);
        layout.set_cloud(vec![cloud("云0"), cloud("云1"), cloud("云2")]);
        assert_eq!(texts(&layout.page(0)), ["本0", "☁云0", "☁云1"]);
        assert_eq!(texts(&layout.page(1)), ["本1", "本2", "本3"]);
    }

    #[test]
    fn sparse_custom_positions_are_layout_cells_not_candidates() {
        let fixed = Candidate {
            kind: CandidateKind::Custom(3),
            ..local("短语")
        };
        let mut layout = CandidateLayout::new(vec![fixed, local("普通")], 5, 2);
        assert_eq!(layout.local().len(), 2);
        assert_eq!(texts(&layout.page(0)), ["普通", "<empty>", "短语"]);
        assert!(layout.candidate(1).is_none());
        assert_eq!(layout.set_cloud(vec![cloud("云")]), 1);
        assert_eq!(texts(&layout.page(0)), ["普通", "<empty>", "短语", "☁云"]);
    }

    #[test]
    fn ninth_position_stays_on_second_page_without_cloud_displacement() {
        let mut layout = CandidateLayout::new(
            vec![Candidate {
                kind: CandidateKind::Custom(9),
                ..local("第九")
            }],
            5,
            2,
        );
        assert_eq!(layout.pages(), 2);
        assert_eq!(layout.capacity(), 0);
        assert_eq!(layout.set_cloud(vec![cloud("云")]), 0);
        assert_eq!(layout.candidate(8).unwrap().text, "第九");
        assert!(layout.candidate(7).is_none());
    }

    #[test]
    fn late_cloud_words_do_not_move_the_highlighted_local_candidate() {
        let layout = CandidateLayout::new(many(12), 9, 2);
        assert_eq!(layout.cloud_insertion_index(), 1);
        assert!(layout.can_insert_cloud(0));
        for index in [1, 2, 7, 9] {
            assert!(!layout.can_insert_cloud(index));
        }
        let empty = CandidateLayout::new(Vec::new(), 9, 2);
        assert!(empty.can_insert_cloud(0));
        assert_eq!(empty.cloud_insertion_index(), 0);
    }

    #[test]
    fn number_two_resolves_the_cloud_word_and_commits_the_whole_pinyin() {
        let mut layout = CandidateLayout::new(many(12), 9, 2);
        let word = Candidate {
            text: "呕心沥血".to_owned(),
            syllables: ["ou", "xin", "li", "xue"].map(String::from).to_vec(),
            ..cloud("云")
        };
        layout.set_cloud(vec![word]);
        let selected = layout.candidate(1).unwrap();
        assert_eq!(selected.text, "呕心沥血");
        assert_eq!(layout.candidate(0).unwrap().text, "本0");
        let mut engine =
            crate::Engine::new(qingjian_dictionary::Dictionary::parse("哦\to\t100\n").unwrap());
        engine.set_input("oxlx");
        assert_eq!(engine.commit(selected), "呕心沥血");
        assert!(engine.composition().is_empty());
    }
}
