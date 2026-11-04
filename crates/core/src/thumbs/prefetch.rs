use std::ops::Range;

/// The entries worth MIME and thumbnail work while `visible` is on screen:
/// its page, the one before and the two after (`[page - 1, page + 2)`).
/// Pages are `page_size` long, or as long as `visible` when that is 0.
pub fn prefetch_range(total: usize, visible: Range<usize>, page_size: usize) -> Range<usize> {
    let page_size = if page_size == 0 {
        visible.len()
    } else {
        page_size
    }
    .max(1);
    let page = visible.start.min(total) / page_size;
    let start = page.saturating_sub(1).saturating_mul(page_size).min(total);
    let end = page.saturating_add(2).saturating_mul(page_size).min(total);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_page_prefetches_two_ahead() {
        assert_eq!(prefetch_range(1000, 0..40, 50), 0..100);
    }

    #[test]
    fn a_middle_page_keeps_one_behind_and_two_ahead() {
        assert_eq!(prefetch_range(1000, 260..300, 50), 200..350);
    }

    #[test]
    fn the_range_is_clamped_to_the_listing() {
        assert_eq!(prefetch_range(120, 110..120, 50), 50..120);
        assert_eq!(prefetch_range(0, 0..0, 50), 0..0);
        assert_eq!(prefetch_range(10, 500..540, 50), 0..10);
    }

    #[test]
    fn a_zero_page_size_uses_the_visible_length() {
        assert_eq!(prefetch_range(1000, 300..330, 0), 270..360);
    }
}
