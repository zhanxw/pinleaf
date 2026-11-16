use crate::coords::PageIndex;

/// Identifies a specific glyph position within a document's reading-order text layout.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageCharPos {
    pub page: PageIndex,
    pub glyph_index: usize,
}

impl PageCharPos {
    #[inline]
    pub const fn new(page: PageIndex, glyph_index: usize) -> Self {
        Self { page, glyph_index }
    }
}

/// Active text selection spanning one or more pages, with drag anchor and focus endpoints.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TextSelectionRange {
    pub anchor: PageCharPos,
    pub focus: PageCharPos,
}

impl TextSelectionRange {
    #[inline]
    pub const fn new(anchor: PageCharPos, focus: PageCharPos) -> Self {
        Self { anchor, focus }
    }

    /// Returns `(start, end)` ordered sequentially in document reading order.
    #[inline]
    pub fn ordered(self) -> (PageCharPos, PageCharPos) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }

    #[inline]
    pub fn is_single_point(self) -> bool {
        self.anchor == self.focus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backward_cross_page_selection_orders_correctly() {
        let sel = TextSelectionRange::new(
            PageCharPos::new(PageIndex(2), 5),
            PageCharPos::new(PageIndex(1), 40),
        );
        let (start, end) = sel.ordered();
        assert_eq!(start, PageCharPos::new(PageIndex(1), 40));
        assert_eq!(end, PageCharPos::new(PageIndex(2), 5));
    }
}
