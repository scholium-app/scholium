//! 有序序列容器：分块向量。
//!
//! 文本 CRDT 需要按 `(位置标识, 字符标识)` 全序保存字符，并支持三类访问：
//!
//! 1. 按可见偏移定位（本地编辑，需要跨过墓碑）；
//! 2. 按位置标识二分插入（远端操作，可能乱序到达）；
//! 3. 就地翻转存活位（删除 / 撤销删除）。
//!
//! 用单块 `Vec` 的中间插入是 O(n) 内存搬移，10 万次动作会变成几十 GB 搬运；用平衡树
//! 又会让 spike 的验证重点从 CRDT 语义变成树实现。分块向量在两者之间：块内是小数组搬移，
//! 块间是线性扫描但常数极小（10 万字符约 200 块）。**块结构不参与 CRDT 语义**，
//! 它只是全序集合的物理布局。

use crate::ids::CharId;
use crate::position::Position;

/// 单个块的软目标大小。超过两倍就分裂。
pub(crate) const CHUNK_TARGET: usize = 512;
/// 分裂阈值。
pub(crate) const CHUNK_SPLIT: usize = CHUNK_TARGET * 2;

/// 序列中的一个字符条目。
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    /// 位置标识，写入后不可变。
    pub(crate) pos: Position,
    /// 字符标识。
    pub(crate) id: CharId,
    /// 字符内容。
    pub(crate) ch: char,
    /// 当前是否存活。删除不物理移除条目，只翻转为 `false`（墓碑）。
    pub(crate) alive: bool,
}

/// 块。
#[derive(Clone, Debug, Default)]
struct Chunk {
    entries: Vec<Entry>,
    /// 本块内存活条目数，用于按可见偏移定位。
    alive: usize,
}

/// 按 `(pos, id)` 升序排列的分块序列。
#[derive(Clone, Debug, Default)]
pub(crate) struct ChunkedSeq {
    chunks: Vec<Chunk>,
}

fn key_order(entry: &Entry, pos: &[u32], id: CharId) -> std::cmp::Ordering {
    entry.pos.as_slice().cmp(pos).then(entry.id.cmp(&id))
}

impl ChunkedSeq {
    /// 空序列。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 从已按序排列的条目构建（快照恢复用）。
    pub(crate) fn from_entries(entries: Vec<Entry>) -> Self {
        let mut seq = Self::new();
        let mut rest = entries;
        while !rest.is_empty() {
            let take = CHUNK_TARGET.min(rest.len());
            let tail = rest.split_off(take);
            let alive = rest.iter().filter(|e| e.alive).count();
            seq.chunks.push(Chunk {
                entries: rest,
                alive,
            });
            rest = tail;
        }
        seq
    }

    /// 全部条目数（含墓碑）。
    pub(crate) fn len(&self) -> usize {
        self.chunks.iter().map(|c| c.entries.len()).sum()
    }

    /// 可见条目数（不含墓碑）。
    pub(crate) fn visible_len(&self) -> usize {
        self.chunks.iter().map(|c| c.alive).sum()
    }

    /// 块数量，用于规模证据。
    pub(crate) fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// 按物理下标读取。
    pub(crate) fn get(&self, index: usize) -> Option<&Entry> {
        let (ci, off) = self.locate(index)?;
        self.chunks[ci].entries.get(off)
    }

    /// 顺序遍历全部条目（含墓碑）。
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.chunks.iter().flat_map(|c| c.entries.iter())
    }

    /// 按物理下标插入。`index == len()` 表示追加到末尾。
    ///
    /// # Panics
    ///
    /// `index > len()` 时 panic：调用方只能插入到 `0..=len`。
    pub(crate) fn insert_at(&mut self, index: usize, entry: Entry) {
        assert!(index <= self.len(), "insert index out of range");
        let alive = entry.alive;
        if self.chunks.is_empty() {
            self.chunks.push(Chunk::default());
        }
        let (ci, off) = self.locate(index).expect("index <= len 时必然可定位");
        self.chunks[ci].entries.insert(off, entry);
        if alive {
            self.chunks[ci].alive += 1;
        }
        if self.chunks[ci].entries.len() > CHUNK_SPLIT {
            self.split(ci);
        }
    }

    /// 翻转物理下标处条目的存活位。返回是否真的发生了变化。
    ///
    /// # Panics
    ///
    /// `index >= len()` 时 panic。
    pub(crate) fn set_alive(&mut self, index: usize, alive: bool) -> bool {
        let (ci, off) = self.locate(index).expect("set_alive 的下标必须存在");
        let entry = &mut self.chunks[ci].entries[off];
        if entry.alive == alive {
            return false;
        }
        entry.alive = alive;
        if alive {
            self.chunks[ci].alive += 1;
        } else {
            self.chunks[ci].alive -= 1;
        }
        true
    }

    /// 可见偏移 → 物理下标。返回物理下标 `p`，使得可见下标小于 `offset` 的字符都排在 `p` 之前，
    /// 可见下标为 `offset` 的字符排在 `p` 或其后。
    ///
    /// `offset == visible_len()` 返回 `len()`（追加位置）。越界返回 `None`。
    pub(crate) fn physical_index_of_visible(&self, offset: usize) -> Option<usize> {
        let mut base = 0usize;
        let mut remaining = offset;
        for chunk in &self.chunks {
            if remaining < chunk.alive {
                for (j, entry) in chunk.entries.iter().enumerate() {
                    if entry.alive {
                        if remaining == 0 {
                            return Some(base + j);
                        }
                        remaining -= 1;
                    }
                }
                return Some(base + chunk.entries.len());
            }
            if remaining == chunk.alive {
                return Some(base + chunk.entries.len());
            }
            remaining -= chunk.alive;
            base += chunk.entries.len();
        }
        if remaining == 0 { Some(base) } else { None }
    }

    /// 首个 `(pos, id)` 不小于给定键的物理下标。
    pub(crate) fn lower_bound(&self, pos: &[u32], id: CharId) -> usize {
        let mut base = 0usize;
        for chunk in &self.chunks {
            match chunk.entries.last() {
                None => {}
                Some(last) if key_order(last, pos, id) == std::cmp::Ordering::Less => {
                    base += chunk.entries.len();
                }
                Some(_) => {
                    let off = chunk
                        .entries
                        .partition_point(|e| key_order(e, pos, id) == std::cmp::Ordering::Less);
                    return base + off;
                }
            }
        }
        base
    }

    /// 按键精确查找。
    pub(crate) fn index_of(&self, pos: &[u32], id: CharId) -> Option<usize> {
        let index = self.lower_bound(pos, id);
        match self.get(index) {
            Some(entry) if entry.id == id && entry.pos.as_slice() == pos => Some(index),
            _ => None,
        }
    }

    fn split(&mut self, ci: usize) {
        let tail = self.chunks[ci].entries.split_off(CHUNK_TARGET);
        let alive = tail.iter().filter(|e| e.alive).count();
        let head_alive = self.chunks[ci].entries.iter().filter(|e| e.alive).count();
        self.chunks[ci].alive = head_alive;
        self.chunks.insert(
            ci + 1,
            Chunk {
                entries: tail,
                alive,
            },
        );
    }

    fn locate(&self, index: usize) -> Option<(usize, usize)> {
        // 空序列没有可定位的条目；`insert_at` 会先建立第一个块再定位。
        if self.chunks.is_empty() {
            return None;
        }
        let total = self.len();
        if index == total {
            // `index == len` 是合法的追加位置：落在最后一个块的末尾。
            let last = self.chunks.len() - 1;
            return Some((last, self.chunks[last].entries.len()));
        }
        let mut base = 0usize;
        for (ci, chunk) in self.chunks.iter().enumerate() {
            let end = base + chunk.entries.len();
            if index < end {
                return Some((ci, index - base));
            }
            base = end;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{ActorId, Id};

    fn entry(seq: u32, pos: u32, ch: char, alive: bool) -> Entry {
        Entry {
            pos: vec![pos],
            id: Id::new(ActorId(1), seq),
            ch,
            alive,
        }
    }

    #[test]
    fn visible_offsets_skip_tombstones() {
        let seq = ChunkedSeq::from_entries(vec![
            entry(0, 1, 'a', true),
            entry(1, 2, 'b', false),
            entry(2, 3, 'c', true),
            entry(3, 4, 'd', false),
        ]);
        assert_eq!(seq.visible_len(), 2);
        assert_eq!(seq.physical_index_of_visible(0), Some(0));
        assert_eq!(seq.physical_index_of_visible(1), Some(2));
        assert_eq!(seq.physical_index_of_visible(2), Some(4));
        assert_eq!(seq.physical_index_of_visible(3), None);
    }

    #[test]
    fn split_preserves_order_and_counts() {
        let entries: Vec<Entry> = (0..3000u32)
            .map(|i| entry(i, i + 1, 'x', i % 3 != 0))
            .collect();
        let expected_alive = entries.iter().filter(|e| e.alive).count();
        let seq = ChunkedSeq::from_entries(entries);
        assert_eq!(seq.len(), 3000);
        assert_eq!(seq.visible_len(), expected_alive);
        for i in 1..seq.len() {
            let prev = seq.get(i - 1).expect("index below len");
            let cur = seq.get(i).expect("index below len");
            assert!(prev.pos < cur.pos, "位置标识必须严格升序");
        }
    }

    #[test]
    fn lower_bound_finds_insertion_point() {
        let seq = ChunkedSeq::from_entries(vec![
            entry(0, 1, 'a', true),
            entry(1, 3, 'c', true),
            entry(2, 5, 'e', true),
        ]);
        // 键是 (位置标识, 字符标识) 全序：用比现有字符都小的标识才能表达 "只看位置"。
        let smallest = Id::new(ActorId(0), 0);
        assert_eq!(seq.lower_bound(&[0], smallest), 0);
        assert_eq!(seq.lower_bound(&[3], smallest), 1);
        assert_eq!(seq.lower_bound(&[6], smallest), 3);
    }
}
