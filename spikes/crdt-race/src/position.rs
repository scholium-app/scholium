//! Logoot 类位置标识。
//!
//! 一个 [`Position`] 是一串 `[0, BASE)` 的数字，按**字典序**比较，且短序列是长序列的前缀时
//! 更小（Rust `Vec<u32>` 的天然顺序）。生成器维持一条不变量：**最后一位永不为 0**。
//!
//! 这条不变量不是审美要求，而是 "两位置之间总能插入" 的前提。若允许 `[1]` 与 `[1, 0]`
//! 同时存在，两者之间不存在任何整数序列（`[1]` 之后、`[1, 0]` 之前的空间为空），
//! 生成器会失败。禁止尾零之后，任何两个合法位置之间都能找到位置：
//!
//! - 首个不同数字处间隙大于 1：直接取中间的随机数字；
//! - 间隙等于 1（相邻数字，或下界已经结束）：延长序列而不是在间隙里取值。
//!
//! 位置标识一旦写入操作就不可变，所以收敛只取决于操作集合，与投递顺序无关。

use crate::error::CrdtError;
use crate::rng::Rng;

/// 数字基数。位置标识的每一层是 `[0, BASE)` 的一个整数。
pub(crate) const BASE: u32 = 1 << 16;

/// 位置标识。
pub(crate) type Position = Vec<u32>;

/// 校验位置标识是否满足不变量（反序列化入口使用）。
pub(crate) fn is_valid(pos: &[u32]) -> bool {
    !pos.is_empty() && pos.last() != Some(&0) && pos.iter().all(|d| *d < BASE)
}

/// 生成严格位于 `a` 与 `b` 之间的新位置。
///
/// `None` 表示该侧无界（`a = None` 是序列开头，`b = None` 是序列结尾）。
///
/// # Errors
///
/// 当 `a >= b`（含两者都是 `Some` 且相等）时返回 [`CrdtError::UnorderedBounds`]。
/// 调用方必须保证上界严格大于下界；位置标识由本函数生成，因此除调用错误外不会触发。
pub(crate) fn between(
    a: Option<&[u32]>,
    b: Option<&[u32]>,
    rng: &mut Rng,
) -> Result<Position, CrdtError> {
    if let (Some(a), Some(b)) = (a, b)
        && a >= b
    {
        return Err(CrdtError::UnorderedBounds);
    }

    let digit = |p: Option<&[u32]>, i: usize| -> u32 { p.and_then(|p| p.get(i).copied()).unwrap_or(0) };
    let digit_upper = |p: Option<&[u32]>, i: usize| -> u32 {
        match p {
            None => BASE,
            Some(p) => p.get(i).copied().unwrap_or(0),
        }
    };

    // 找到第一个数字不同的下标。双方都已结束时停在末尾（此时两侧都无界或已比较完毕）。
    let mut k = 0usize;
    loop {
        let in_a = a.is_some_and(|p| k < p.len());
        let in_b = b.is_some_and(|p| k < p.len());
        if !in_a && !in_b {
            break;
        }
        if digit(a, k) != digit_upper(b, k) {
            break;
        }
        k += 1;
    }

    let lo = digit(a, k);
    let hi = digit_upper(b, k);
    if hi <= lo {
        return Err(CrdtError::UnorderedBounds);
    }

    // 下界的前 k 位（不足部分补 0）。这些位在上界之下，作为新位置的前缀是安全的。
    let mut out: Position = a.map(|p| p[..k.min(p.len())].to_vec()).unwrap_or_default();
    out.resize(k, 0);

    if hi - lo > 1 {
        let d = rng.range_inclusive(u64::from(lo) + 1, u64::from(hi) - 1) as u32;
        out.push(d);
        return Ok(out);
    }

    // 相邻数字：本层取不到整数，必须延长序列。
    let a_len = a.map_or(0, |p| p.len());
    if a_len > k {
        // 下界在第 k 位之后还有数字，因此必须整体保留它，再在尾部追加。
        out = a.expect("a_len > k 蕴含 a 存在").to_vec();
    } else {
        // 下界在下标 k 处已经结束：先补一个 0，再把新数字放在更深一层。
        out.resize(k + 1, 0);
    }
    let d = rng.range_inclusive(1, u64::from(BASE) - 1) as u32;
    out.push(d);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(digits: &[u32]) -> Position {
        digits.to_vec()
    }

    #[test]
    fn generated_position_lies_strictly_between_bounds() {
        let mut rng = Rng::new(7);
        let cases: [(Option<Position>, Option<Position>); 7] = [
            (None, None),
            (None, Some(pos(&[1]))),
            (None, Some(pos(&[0, 1]))),
            (Some(pos(&[1])), None),
            (Some(pos(&[BASE - 1])), None),
            (Some(pos(&[1])), Some(pos(&[1, 1]))),
            (Some(pos(&[1])), Some(pos(&[1, 0, 5]))),
        ];
        for (a, b) in cases {
            for _ in 0..64 {
                let mid = between(a.as_deref(), b.as_deref(), &mut rng).expect("bounds are ordered");
                assert!(is_valid(&mid), "generated position must satisfy invariant: {mid:?}");
                if let Some(a) = a.as_deref() {
                    assert!(*a < mid[..], "mid must be greater than lower bound: {a:?} < {mid:?}");
                }
                if let Some(b) = b.as_deref() {
                    assert!(mid[..] < *b, "mid must be less than upper bound: {mid:?} < {b:?}");
                }
            }
        }
    }

    #[test]
    fn repeated_insertion_at_same_gap_stays_ordered() {
        let mut rng = Rng::new(11);
        let mut lower = pos(&[5]);
        let upper = pos(&[6]);
        for _ in 0..512 {
            let mid = between(Some(&lower), Some(&upper), &mut rng).expect("bounds are ordered");
            assert!(lower < mid && mid < upper);
            lower = mid;
        }
    }

    #[test]
    fn equal_bounds_are_rejected() {
        let mut rng = Rng::new(3);
        let same = pos(&[2]);
        assert!(between(Some(&same), Some(&same), &mut rng).is_err());
    }
}
