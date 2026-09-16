//! 确定性伪随机数发生器。
//!
//! 不引入 `rand`：判据要求 "固定随机种子可复现"，自带的 SplitMix64 状态可以在快照里
//! 原样存取，`rand` 的多种 RNG 还要额外处理版本差异。

/// SplitMix64。周期 2^64，输出经过两轮乘法混合，足够随机化夹具与位置标识。
#[derive(Clone, Debug)]
pub(crate) struct Rng {
    state: u64,
}

impl Rng {
    /// 以种子构造。种子 0 也会得到非退化状态。
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15),
        }
    }

    /// 从快照恢复状态。
    pub(crate) fn from_state(state: u64) -> Self {
        Self { state }
    }

    /// 内部状态，用于快照。
    pub(crate) fn state(&self) -> u64 {
        self.state
    }

    /// 下一个 64 位输出。
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `[0, bound)` 内的值。
    ///
    /// # Panics
    ///
    /// `bound == 0` 时 panic：调用方必须先排除空区间。
    pub(crate) fn below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0, "below(0) has no valid output");
        self.next_u64() % bound
    }

    /// `[lo, hi]` 闭区间内的值。
    ///
    /// # Panics
    ///
    /// `lo > hi` 时 panic。
    pub(crate) fn range_inclusive(&mut self, lo: u64, hi: u64) -> u64 {
        assert!(lo <= hi, "range_inclusive requires lo <= hi");
        lo + self.below(hi - lo + 1)
    }

    /// 以百分比概率返回 true。
    pub(crate) fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// Fisher-Yates 就地洗牌，用于制造乱序投递。
    pub(crate) fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            items.swap(i, j);
        }
    }
}
