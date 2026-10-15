//! Pure width decisions for the bars, kept free of egui so they are unit-tested.

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

/// Widths of the focus bar fields that are shown. Fields keep their natural width when
/// they fit; otherwise the longest ones are cut to a common cap (never below `min`), and
/// trailing fields are dropped when even that cannot fit.
pub(crate) fn fit_fields(natural: &[f32], gap: f32, min: f32, available: f32) -> Vec<f32> {
    for count in (1..=natural.len()).rev() {
        let fields = &natural[..count];
        let budget = available - gap * (count - 1) as f32;
        if fields.iter().sum::<f32>() <= budget {
            return fields.to_vec();
        }
        if min * count as f32 <= budget {
            let cap = cap_for(fields, budget, min);
            return fields.iter().map(|w| w.min(cap)).collect();
        }
    }
    Vec::new()
}

/// The largest common cap so that the capped widths still fit `budget`.
fn cap_for(fields: &[f32], budget: f32, min: f32) -> f32 {
    let (mut low, mut high) = (min, fields.iter().copied().fold(min, f32::max));
    for _ in 0..24 {
        let mid = (low + high) / 2.0;
        if fields.iter().map(|w| w.min(mid)).sum::<f32>() <= budget {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

#[cfg(test)]
mod tests {
    use super::{fit_fields, fit_groups};

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

    #[test]
    fn fields_keep_their_width_when_there_is_room() {
        assert_eq!(
            fit_fields(&[80.0, 100.0], 8.0, 60.0, 400.0),
            vec![80.0, 100.0]
        );
    }

    #[test]
    fn the_longest_field_is_cut_first() {
        let fitted = fit_fields(&[80.0, 200.0, 90.0], 8.0, 60.0, 300.0);
        let total: f32 = fitted.iter().sum::<f32>() + 16.0;
        assert!(total <= 300.01, "{fitted:?}");
        assert_eq!(fitted[0], 80.0);
        assert_eq!(fitted[2], 90.0);
        assert!(fitted[1] < 200.0 && fitted[1] >= 60.0);
    }

    #[test]
    fn trailing_fields_are_dropped_when_even_the_minimum_does_not_fit() {
        let fitted = fit_fields(&[80.0, 80.0, 80.0], 8.0, 60.0, 140.0);
        assert_eq!(fitted.len(), 2);
        assert!(fit_fields(&[80.0], 8.0, 60.0, 10.0).is_empty());
    }
}
