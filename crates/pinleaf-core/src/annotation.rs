use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::coords::{NormPoint, NormQuad, NormRect, PageIndex};

/// Unique identifier for an annotation (persisted in PDF `/NM` entry).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnnotationId(pub Uuid);

impl AnnotationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_pdf_nm(nm: &str) -> Option<Self> {
        let stripped = nm
            .strip_prefix("pinleaf:")
            .or_else(|| nm.strip_prefix("pdf-reader:"))
            .unwrap_or(nm);
        Uuid::parse_str(stripped).ok().map(Self)
    }

    pub fn to_pdf_nm(self) -> String {
        format!("pinleaf:{}", self.0)
    }
}

impl Default for AnnotationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalized RGBA color (`0.0..=1.0`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl RgbaColor {
    pub const YELLOW: Self = Self::rgb(1.0, 0.90, 0.20);
    pub const GREEN: Self = Self::rgb(0.45, 0.88, 0.45);
    pub const BLUE: Self = Self::rgb(0.42, 0.72, 1.0);
    pub const PINK: Self = Self::rgb(0.98, 0.55, 0.74);
    pub const ORANGE: Self = Self::rgb(1.0, 0.68, 0.25);

    pub const PRESETS: [(&'static str, Self); 5] = [
        ("Yellow", Self::YELLOW),
        ("Green", Self::GREEN),
        ("Blue", Self::BLUE),
        ("Pink", Self::PINK),
        ("Orange", Self::ORANGE),
    ];

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 0.36 }
    }

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_u32_rgba(self, alpha_override: Option<f32>) -> u32 {
        let r = (self.r.clamp(0.0, 1.0) * 255.0).round() as u32;
        let g = (self.g.clamp(0.0, 1.0) * 255.0).round() as u32;
        let b = (self.b.clamp(0.0, 1.0) * 255.0).round() as u32;
        let a = (alpha_override.unwrap_or(self.a).clamp(0.0, 1.0) * 255.0).round() as u32;
        (r << 24) | (g << 16) | (b << 8) | a
    }
}

pub fn normalize_category_char(ch: char) -> Option<char> {
    if ch.is_ascii_alphabetic() {
        Some(ch.to_ascii_lowercase())
    } else {
        None
    }
}

pub fn highlight_category_spec(ch: char) -> (&'static str, RgbaColor) {
    match ch.to_ascii_lowercase() {
        'a' => ("Assumption", RgbaColor::rgb(0.96, 0.84, 0.34)),
        'b' => ("Background", RgbaColor::rgb(0.64, 0.78, 0.96)),
        'c' => ("Claim", RgbaColor::rgb(0.98, 0.62, 0.42)),
        'd' => ("Definition", RgbaColor::rgb(0.45, 0.85, 0.78)),
        'e' => ("Equation", RgbaColor::rgb(0.76, 0.64, 0.98)),
        'f' => ("Figure", RgbaColor::rgb(0.54, 0.88, 0.56)),
        'g' => ("Goal", RgbaColor::rgb(0.96, 0.78, 0.30)),
        'h' => ("Highlight", RgbaColor::YELLOW),
        'i' => ("Important", RgbaColor::rgb(0.99, 0.48, 0.48)),
        'j' => ("Note", RgbaColor::rgb(0.84, 0.86, 0.90)),
        'k' => ("Key Idea", RgbaColor::rgb(1.0, 0.82, 0.28)),
        'l' => ("Limitation", RgbaColor::rgb(0.95, 0.54, 0.68)),
        'm' => ("Method", RgbaColor::rgb(0.44, 0.76, 0.98)),
        'n' => ("Novelty", RgbaColor::rgb(0.62, 0.92, 0.46)),
        'o' => ("Observation", RgbaColor::rgb(0.98, 0.72, 0.48)),
        'p' => ("Problem", RgbaColor::rgb(0.96, 0.46, 0.56)),
        'q' => ("Question", RgbaColor::rgb(0.88, 0.60, 0.96)),
        'r' => ("Result", RgbaColor::rgb(0.40, 0.86, 0.54)),
        's' => ("Summary", RgbaColor::rgb(0.56, 0.86, 0.94)),
        't' => ("Theorem", RgbaColor::rgb(0.46, 0.66, 0.98)),
        'u' => ("Open Issue", RgbaColor::rgb(0.96, 0.64, 0.36)),
        'v' => ("Verification", RgbaColor::rgb(0.50, 0.90, 0.72)),
        'w' => ("Related Work", RgbaColor::rgb(0.74, 0.78, 0.96)),
        'x' => ("Counterexample", RgbaColor::rgb(0.98, 0.42, 0.42)),
        'y' => ("Hypothesis", RgbaColor::rgb(0.92, 0.74, 0.96)),
        'z' => ("Takeaway", RgbaColor::rgb(0.98, 0.86, 0.38)),
        _ => ("Highlight", RgbaColor::YELLOW),
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AnnotationKind {
    Highlight,
    Underline,
    StrikeOut,
    /// PDF `/Subtype /Square` rectangular highlight around figures or equations.
    AreaNote,
    /// PDF `/Subtype /Text` point-anchored sticky note.
    StickyNote,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub id: AnnotationId,
    /// Existing `lopdf::ObjectId` (`(obj_num, gen_num)`) if loaded from or already saved to PDF.
    pub pdf_object_id: Option<(u32, u16)>,
    pub page: PageIndex,
    pub kind: AnnotationKind,
    pub quads: Vec<NormQuad>,
    pub bounds: NormRect,
    pub color: RgbaColor,
    /// Optional single-letter highlight category (`a`..`z`), persisted in PDF `/Subj (pinleaf-cat:<ch>)`.
    pub category: Option<char>,
    pub selected_text: Option<String>,
    pub note_contents: Option<String>,
    pub modified_at: String,
}

impl Annotation {
    pub fn hit_test(&self, pt: NormPoint, tolerance_pts: f32) -> bool {
        if !self.bounds.inflate(tolerance_pts).contains(pt) {
            return false;
        }
        if self.quads.is_empty() {
            return true;
        }
        self.quads
            .iter()
            .any(|q| q.bounding_rect().inflate(tolerance_pts).contains(pt))
    }
}

/// Changes accumulated since the last PDF load or save.
#[derive(Clone, Debug, Default)]
pub struct AnnotationDiff {
    pub upserted: Vec<Annotation>,
    pub deleted_pdf_objs: Vec<(PageIndex, (u32, u16))>,
}

impl AnnotationDiff {
    pub fn is_empty(&self) -> bool {
        self.upserted.is_empty() && self.deleted_pdf_objs.is_empty()
    }
}

#[derive(Clone, Debug)]
enum AnnotationOp {
    Insert(Annotation),
    Delete(Annotation),
    Replace { before: Annotation, after: Annotation },
}

/// Central single-source-of-truth annotation store shared by all panes and windows of a document.
#[derive(Clone, Debug, Default)]
pub struct AnnotationStore {
    items: HashMap<AnnotationId, Annotation>,
    dirty_upserts: HashSet<AnnotationId>,
    deleted_pdf_objs: Vec<(PageIndex, (u32, u16))>,
    undo_stack: Vec<AnnotationOp>,
    redo_stack: Vec<AnnotationOp>,
}

impl AnnotationStore {
    pub fn from_loaded(loaded: Vec<Annotation>) -> Self {
        let mut items = HashMap::with_capacity(loaded.len());
        for ann in loaded {
            items.insert(ann.id, ann);
        }
        Self {
            items,
            ..Self::default()
        }
    }

    pub fn is_dirty(&self) -> bool {
        !self.dirty_upserts.is_empty() || !self.deleted_pdf_objs.is_empty()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn get(&self, id: AnnotationId) -> Option<&Annotation> {
        self.items.get(&id)
    }

    pub fn for_page(&self, page: PageIndex) -> Vec<&Annotation> {
        let mut list: Vec<&Annotation> = self
            .items
            .values()
            .filter(|a| a.page == page)
            .collect();
        list.sort_by(|a, b| {
            a.bounds
                .min_y
                .total_cmp(&b.bounds.min_y)
                .then_with(|| a.bounds.min_x.total_cmp(&b.bounds.min_x))
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        list
    }

    pub fn all_sorted(&self) -> Vec<&Annotation> {
        let mut list: Vec<&Annotation> = self.items.values().collect();
        list.sort_by(|a, b| {
            a.page
                .cmp(&b.page)
                .then_with(|| a.bounds.min_y.total_cmp(&b.bounds.min_y))
                .then_with(|| a.bounds.min_x.total_cmp(&b.bounds.min_x))
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        list
    }

    pub fn hit_test_page(&self, page: PageIndex, pt: NormPoint, tolerance_pts: f32) -> Option<&Annotation> {
        self.for_page(page)
            .into_iter()
            .rev()
            .find(|ann| ann.hit_test(pt, tolerance_pts))
    }

    pub fn insert(&mut self, ann: Annotation) {
        let id = ann.id;
        self.items.insert(id, ann.clone());
        self.dirty_upserts.insert(id);
        self.undo_stack.push(AnnotationOp::Insert(ann));
        self.redo_stack.clear();
    }

    pub fn update_color(&mut self, id: AnnotationId, color: RgbaColor, modified_at: String) -> bool {
        let Some(existing) = self.items.get(&id).cloned() else {
            return false;
        };
        self.update_category_and_color(id, existing.category, color, modified_at)
    }

    pub fn update_category_and_color(
        &mut self,
        id: AnnotationId,
        category: Option<char>,
        color: RgbaColor,
        modified_at: String,
    ) -> bool {
        let Some(existing) = self.items.get(&id).cloned() else {
            return false;
        };
        let mut after = existing.clone();
        after.category = category.and_then(normalize_category_char);
        after.color = color;
        after.modified_at = modified_at;
        self.items.insert(id, after.clone());
        self.dirty_upserts.insert(id);
        self.undo_stack.push(AnnotationOp::Replace {
            before: existing,
            after,
        });
        self.redo_stack.clear();
        true
    }

    pub fn update_note(&mut self, id: AnnotationId, note: Option<String>, modified_at: String) -> bool {
        let Some(existing) = self.items.get(&id).cloned() else {
            return false;
        };
        let mut after = existing.clone();
        after.note_contents = note.filter(|s| !s.trim().is_empty());
        after.modified_at = modified_at;
        self.items.insert(id, after.clone());
        self.dirty_upserts.insert(id);
        self.undo_stack.push(AnnotationOp::Replace {
            before: existing,
            after,
        });
        self.redo_stack.clear();
        true
    }

    pub fn delete(&mut self, id: AnnotationId) -> bool {
        let Some(existing) = self.items.remove(&id) else {
            return false;
        };
        self.dirty_upserts.remove(&id);
        if let Some(obj_id) = existing.pdf_object_id {
            self.deleted_pdf_objs.push((existing.page, obj_id));
        }
        self.undo_stack.push(AnnotationOp::Delete(existing));
        self.redo_stack.clear();
        true
    }

    pub fn undo(&mut self) -> bool {
        let Some(op) = self.undo_stack.pop() else {
            return false;
        };
        match op {
            AnnotationOp::Insert(ann) => {
                self.items.remove(&ann.id);
                self.dirty_upserts.remove(&ann.id);
                if let Some(obj_id) = ann.pdf_object_id {
                    self.deleted_pdf_objs.push((ann.page, obj_id));
                }
                self.redo_stack.push(AnnotationOp::Insert(ann));
            }
            AnnotationOp::Delete(ann) => {
                if let Some(obj_id) = ann.pdf_object_id {
                    self.remove_deleted_tracking(ann.page, obj_id);
                }
                self.dirty_upserts.insert(ann.id);
                self.items.insert(ann.id, ann.clone());
                self.redo_stack.push(AnnotationOp::Delete(ann));
            }
            AnnotationOp::Replace { before, after } => {
                self.items.insert(before.id, before.clone());
                self.dirty_upserts.insert(before.id);
                self.redo_stack.push(AnnotationOp::Replace { before, after });
            }
        }
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo_stack.pop() else {
            return false;
        };
        match op {
            AnnotationOp::Insert(ann) => {
                if let Some(obj_id) = ann.pdf_object_id {
                    self.remove_deleted_tracking(ann.page, obj_id);
                }
                self.dirty_upserts.insert(ann.id);
                self.items.insert(ann.id, ann.clone());
                self.undo_stack.push(AnnotationOp::Insert(ann));
            }
            AnnotationOp::Delete(ann) => {
                self.items.remove(&ann.id);
                self.dirty_upserts.remove(&ann.id);
                if let Some(obj_id) = ann.pdf_object_id {
                    self.deleted_pdf_objs.push((ann.page, obj_id));
                }
                self.undo_stack.push(AnnotationOp::Delete(ann));
            }
            AnnotationOp::Replace { before, after } => {
                self.items.insert(after.id, after.clone());
                self.dirty_upserts.insert(after.id);
                self.undo_stack.push(AnnotationOp::Replace { before, after });
            }
        }
        true
    }

    pub fn pending_diff(&self) -> AnnotationDiff {
        let upserted = self
            .dirty_upserts
            .iter()
            .filter_map(|id| self.items.get(id).cloned())
            .collect();
        AnnotationDiff {
            upserted,
            deleted_pdf_objs: self.deleted_pdf_objs.clone(),
        }
    }

    /// Clears dirty tracking after a successful save and updates newly assigned PDF ObjectIds.
    pub fn mark_saved(&mut self, assigned_ids: &[(AnnotationId, (u32, u16))]) {
        for &(id, obj_id) in assigned_ids {
            if let Some(ann) = self.items.get_mut(&id) {
                ann.pdf_object_id = Some(obj_id);
            }
        }
        self.dirty_upserts.clear();
        self.deleted_pdf_objs.clear();
        // Keep undo history coherent if items were assigned PDF ObjectIds.
        for op in self.undo_stack.iter_mut().chain(self.redo_stack.iter_mut()) {
            match op {
                AnnotationOp::Insert(a) | AnnotationOp::Delete(a) => {
                    if let Some(&(_, obj_id)) = assigned_ids.iter().find(|&&(id, _)| id == a.id) {
                        a.pdf_object_id = Some(obj_id);
                    }
                }
                AnnotationOp::Replace { before, after } => {
                    if let Some(&(_, obj_id)) = assigned_ids.iter().find(|&&(id, _)| id == before.id) {
                        before.pdf_object_id = Some(obj_id);
                        after.pdf_object_id = Some(obj_id);
                    }
                }
            }
        }
    }

    fn remove_deleted_tracking(&mut self, page: PageIndex, obj_id: (u32, u16)) {
        if let Some(pos) = self
            .deleted_pdf_objs
            .iter()
            .position(|&(p, o)| p == page && o == obj_id)
        {
            self.deleted_pdf_objs.swap_remove(pos);
        }
    }
}

/// Formats the current UTC timestamp in PDF date string format `D:YYYYMMDDHHmmSSZ`.
pub fn pdf_now_string() -> String {
    // Simple deterministic-free epoch->UTC converter without extra crate deps.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;
    let (year, month, day) = civil_from_days(days as i64);
    format!("D:{year:04}{month:02}{day:02}{hour:02}{min:02}{sec:02}Z")
}

// Howard Hinnant's civil-from-days algorithm (public domain).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ann(page: u32, y: f32, pdf_obj: Option<(u32, u16)>) -> Annotation {
        let bounds = NormRect::new(50.0, y, 250.0, y + 14.0);
        Annotation {
            id: AnnotationId::new(),
            pdf_object_id: pdf_obj,
            page: PageIndex(page),
            kind: AnnotationKind::Highlight,
            quads: vec![NormQuad::from_rect(bounds)],
            bounds,
            color: RgbaColor::YELLOW,
            category: None,
            selected_text: Some("sample quote".to_string()),
            note_contents: None,
            modified_at: "D:20261116100000Z".to_string(),
        }
    }

    #[test]
    fn crud_undo_redo_and_diff() {
        let loaded = sample_ann(0, 100.0, Some((42, 0)));
        let loaded_id = loaded.id;
        let mut store = AnnotationStore::from_loaded(vec![loaded]);
        assert!(!store.is_dirty());

        let added = sample_ann(1, 200.0, None);
        let added_id = added.id;
        store.insert(added);
        assert!(store.is_dirty());
        assert_eq!(store.pending_diff().upserted.len(), 1);

        store.update_note(added_id, Some("Important theorem".into()), "D:2026Z".into());
        assert_eq!(
            store.get(added_id).unwrap().note_contents.as_deref(),
            Some("Important theorem")
        );

        assert!(store.delete(loaded_id));
        assert_eq!(store.pending_diff().deleted_pdf_objs, vec![(PageIndex(0), (42, 0))]);
        assert!(store.get(loaded_id).is_none());

        // Undo delete -> restores loaded
        assert!(store.undo());
        assert!(store.get(loaded_id).is_some());
        assert!(store.pending_diff().deleted_pdf_objs.is_empty());

        // Redo delete -> removes loaded again
        assert!(store.redo());
        assert!(store.get(loaded_id).is_none());
        assert_eq!(store.pending_diff().deleted_pdf_objs, vec![(PageIndex(0), (42, 0))]);

        store.mark_saved(&[(added_id, (99, 0))]);
        assert!(!store.is_dirty());
        assert_eq!(store.get(added_id).unwrap().pdf_object_id, Some((99, 0)));
    }

    #[test]
    fn highlight_categories_cover_a_to_z() {
        for ch in 'a'..='z' {
            let norm = normalize_category_char(ch.to_ascii_uppercase()).unwrap();
            assert_eq!(norm, ch);
            let (label, color) = highlight_category_spec(ch);
            assert!(!label.is_empty());
            assert!(color.a > 0.0);
        }
        assert!(normalize_category_char('5').is_none());
    }
}
