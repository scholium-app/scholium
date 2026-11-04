//! Pure width decisions for the bars, kept free of egui so they are unit-tested.

/// Leading status segments have higher priority; only complete segments may fit.
pub(crate) fn fit_status_segments(widths: &[f32], separator: f32, available: f32) -> usize {
    (0..=widths.len())
        .rev()
        .find(|&n| {
            widths[..n].iter().sum::<f32>() + separator * n.saturating_sub(1) as f32 <= available
        })
        .unwrap_or(0)
}

/// How many leading groups fit in `available`. `widths` are the groups' widths, `sep` the
/// full width between two neighbours (separator plus spacing) and `overflow` the width of
/// the "»" button that appears, after one more separator, as soon as any group is hidden.
pub(crate) fn fit_groups(widths: &[f32], sep: f32, overflow: f32, available: f32) -> usize {
    let used = |count: usize| -> f32 {
        widths[..count].iter().sum::<f32>() + sep * count.saturating_sub(1) as f32
    };
    if used(widths.len()) <= available {
        return widths.len();
    }
    (0..widths.len())
        .rev()
        .find(|&count| {
            let lead = if count > 0 { sep } else { 0.0 };
            used(count) + lead + overflow <= available
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{fit_groups, fit_status_segments};

    #[test]
    fn status_drops_context_then_words_then_page_as_whole_segments() {
        let widths = [60.0, 50.0, 30.0]; // page, words, context
        for (room, count) in [(160.0, 3), (159.0, 2), (119.0, 1), (59.0, 0)] {
            assert_eq!(fit_status_segments(&widths, 10.0, room), count);
        }
    }

    const WIDTHS: [f32; 4] = [40.0, 150.0, 130.0, 58.0];

    #[test]
    fn every_group_stays_when_it_fits() {
        assert_eq!(fit_groups(&WIDTHS, 17.0, 28.0, 1000.0), 4);
    }

    #[test]
    fn the_last_groups_move_into_overflow_in_order() {
        // 40+17+150+17+130 = 354 plus separator and the overflow button = 399.
        assert_eq!(fit_groups(&WIDTHS, 17.0, 28.0, 399.0), 3);
        assert_eq!(fit_groups(&WIDTHS, 17.0, 28.0, 398.0), 2);
        assert_eq!(fit_groups(&WIDTHS, 17.0, 28.0, 100.0), 1);
        assert_eq!(fit_groups(&WIDTHS, 17.0, 28.0, 50.0), 0);
    }

    #[test]
    fn nothing_fits_means_everything_overflows() {
        assert_eq!(fit_groups(&WIDTHS, 17.0, 28.0, 10.0), 0);
    }
}
