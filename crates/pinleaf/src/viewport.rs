use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Axis, Bounds, ClipboardItem, ContentMask, Context, CursorStyle, Entity,
    EventEmitter, FocusHandle, FontWeight, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, PinchEvent, Point, Render, ScrollWheelEvent,
    Window, canvas, div, point, prelude::*, px, rgb, rgba, size,
};
use pinleaf_core::{
    Annotation, AnnotationId, AnnotationKind, FreezeState, JumpHistory, NormPoint, NormQuad,
    NormRect, PageCharPos, PageIndex, ReadingAnchor, RgbaColor, TextSelectionRange, ViewportId,
    ZoomSpec, annotation::pdf_now_string, highlight_category_spec, normalize_category_char,
};
use pinleaf_engine::{
    LinkRegion, LinkTarget, collect_page_smart_refs, resolve_smart_jump_at,
};

use crate::actions::{
    AnnotateWithNote, AutoScrollFaster, AutoScrollSlower, BeginHintJump, BeginHintOverview,
    BeginHintSelect, BeginJumpMark, BeginSetMark, BeginTypedHighlight, ClosePane, CopySelection,
    DeleteNearestPortal, DetachPaneToWindow, FirstPage, FreezeAndNewWindow, FreezeAndSplitDown,
    FreezeAndSplitRight, HINT_MODE_CONTEXT, HighlightSelection, JumpBack, JumpForward, LastPage,
    LinkPortal, LookupScholar, MARK_PENDING_CONTEXT, NOTE_EDITOR_CONTEXT, NextPage, PrevPage,
    ScrollHalfPageDown, ScrollHalfPageUp, ScrollLineDown, ScrollLineUp, ScrollScreenDown,
    ScrollScreenUp, SynctexInverseJump,
    ToggleAreaMode, ToggleAutoScroll, ToggleFreeze, TogglePortalFollower, ToggleReadingRuler,
    VIEWPORT_CONTEXT, ZoomFitContentWidth, ZoomFitWidth, ZoomIn, ZoomOut,
};
use crate::app_state::{AppRegistry, focus_viewport_globally};
use crate::external::{ExtensionContext, extract_first_doi, run_synctex_inverse_editor};
use crate::session::{ColorMode, DocumentSession, scale_to_zoom_bucket};

const PAGE_GAP_PX: f32 = 16.0;
const TOP_PAD_PX: f32 = 14.0;
const SIDE_MARGIN_PX: f32 = 18.0;

#[derive(Clone, Debug)]
pub enum ViewportEvent {
    Focused(ViewportId),
    RequestFreezeAndSplit { id: ViewportId, axis: Axis },
    RequestFreezeAndNewWindow(ViewportId),
    RequestDetachToWindow(ViewportId),
    RequestClose(ViewportId),
    RequestSmartLinkRoute {
        source_id: ViewportId,
        page: PageIndex,
        top_norm_y: Option<f32>,
    },
    ReadingPositionChanged {
        id: ViewportId,
        page: PageIndex,
        focal_y: f32,
    },
    RequestEnsurePortalFollower {
        source_id: ViewportId,
        dst: ReadingAnchor,
    },
    RequestTogglePortalFollower(ViewportId),
    RequestGoToPage(ViewportId),
}

#[derive(Clone, Debug)]
struct PagePlacement {
    page: PageIndex,
    norm_width: f32,
    norm_height: f32,
    local_x: f32,
    local_y: f32,
    pixel_w: f32,
    pixel_h: f32,
}

#[derive(Clone, Debug, Default)]
enum DragMode {
    #[default]
    None,
    TextSelect {
        anchor: PageCharPos,
    },
    AreaMarquee {
        page: PageIndex,
        start: NormPoint,
        current: NormPoint,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingMarkOp {
    Set,
    Jump,
    TypedHighlight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintModeKind {
    Jump,
    Overview,
    SelectStart,
    SelectEnd { start: PageCharPos },
}

#[derive(Clone, Debug)]
enum HintAction {
    InternalJump { page: PageIndex, top_norm_y: f32, label: String },
    ExternalUrl(String),
    SelectChar(PageCharPos),
}

#[derive(Clone, Debug)]
struct HintCandidate {
    label: String,
    page: PageIndex,
    rect: NormRect,
    action: HintAction,
}

#[derive(Clone, Debug)]
struct HintModeState {
    kind: HintModeKind,
    typed: String,
    candidates: Vec<HintCandidate>,
}

const HINT_ALPHABET: &[u8] = b"asdfghjklqwertyuiopzxcvbnm";

/// Generates unique home-row-first 2-letter keyboard hint labels (`aa`, `as`, `ad`, ...).
pub fn generate_hint_labels(count: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(count);
    let len = HINT_ALPHABET.len();
    for i in 0..count {
        if i >= len * len {
            break;
        }
        let a = HINT_ALPHABET[i / len] as char;
        let b = HINT_ALPHABET[i % len] as char;
        let mut s = String::with_capacity(2);
        s.push(a);
        s.push(b);
        out.push(s);
    }
    out
}

#[derive(Clone, Debug)]
struct OverviewPreview {
    label: String,
    page: PageIndex,
    top_norm_y: f32,
    anchor_win_pt: Point<Pixels>,
}

pub struct PdfViewport {
    pub id: ViewportId,
    pub session: Entity<DocumentSession>,
    pub anchor: ReadingAnchor,
    pub freeze_state: FreezeState,
    pub portal_follower: bool,
    pub active_portal_label: Option<String>,
    pub hide_header: bool,
    pub pan_x: f32,
    pub jump_history: JumpHistory,
    focus_handle: FocusHandle,
    note_focus_handle: FocusHandle,
    last_canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    drag_mode: DragMode,
    selection: Option<TextSelectionRange>,
    area_mode: bool,
    pending_mark_op: Option<PendingMarkOp>,
    page_count_prefix: String,
    hint_mode: Option<HintModeState>,
    ruler_line: Option<(PageIndex, usize)>,
    overview_preview: Option<OverviewPreview>,
    hovered_link: Option<(PageIndex, usize)>,
    inspected_annotation: Option<AnnotationId>,
    editing_note: Option<(AnnotationId, String)>,
    frozen_feedback_active: bool,
    auto_scroll_speed: Option<f32>,
    auto_scroll_loop_active: bool,
    _session_obs: gpui::Subscription,
}

impl EventEmitter<ViewportEvent> for PdfViewport {}

impl PdfViewport {
    pub fn new(
        id: ViewportId,
        session: Entity<DocumentSession>,
        anchor: ReadingAnchor,
        freeze_state: FreezeState,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let note_focus_handle = cx.focus_handle();
        let _session_obs = cx.observe(&session, |_, _, cx| cx.notify());
        Self {
            id,
            session,
            anchor,
            freeze_state,
            portal_follower: false,
            active_portal_label: None,
            hide_header: false,
            pan_x: 0.0,
            jump_history: JumpHistory::default(),
            focus_handle,
            note_focus_handle,
            last_canvas_bounds: Rc::new(Cell::new(Bounds::default())),
            drag_mode: DragMode::None,
            selection: None,
            area_mode: false,
            pending_mark_op: None,
            page_count_prefix: String::new(),
            hint_mode: None,
            ruler_line: None,
            overview_preview: None,
            hovered_link: None,
            inspected_annotation: None,
            editing_note: None,
            frozen_feedback_active: false,
            auto_scroll_speed: None,
            auto_scroll_loop_active: false,
            _session_obs,
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub fn set_session(&mut self, session: Entity<DocumentSession>, cx: &mut Context<Self>) {
        self._session_obs = cx.observe(&session, |_, _, cx| cx.notify());
        self.session = session;
        self.anchor = ReadingAnchor::default();
        self.portal_follower = false;
        self.active_portal_label = None;
        self.pan_x = 0.0;
        self.jump_history = JumpHistory::default();
        self.selection = None;
        self.pending_mark_op = None;
        self.page_count_prefix.clear();
        self.hint_mode = None;
        self.ruler_line = None;
        self.overview_preview = None;
        self.inspected_annotation = None;
        self.editing_note = None;
        self.drag_mode = DragMode::None;
        self.auto_scroll_speed = None;
        cx.notify();
    }

    /// Resolves the current effective scale factor in logical pixels per PDF point.
    pub fn current_effective_scale(&self, cx: &App) -> f32 {
        let bounds = self.last_canvas_bounds.get();
        let vw = bounds.size.width.as_f32().max(320.0);
        let vh = bounds.size.height.as_f32().max(240.0);
        self.resolve_scale(vw, vh, cx)
    }

    fn resolve_scale(&self, vw: f32, vh: f32, cx: &App) -> f32 {
        let metas = &self.session.read(cx).page_metas;
        let meta = metas
            .get(self.anchor.page.as_usize())
            .or_else(|| metas.first());
        let (pw, ph) = meta
            .map(|m| (m.norm_width, m.norm_height))
            .unwrap_or((612.0, 792.0));
        match self.anchor.zoom {
            ZoomSpec::Fixed(s) => s.clamp(0.25, 4.5),
            ZoomSpec::FitWidth => ((vw - SIDE_MARGIN_PX * 2.0).max(160.0) / pw).clamp(0.25, 4.5),
            ZoomSpec::FitContentWidth => {
                let (cx0, cx1) = self.session.read(cx).content_x_bounds_read(self.anchor.page);
                let content_w = (cx1 - cx0).max(140.0);
                ((vw - SIDE_MARGIN_PX * 2.0).max(160.0) / content_w).clamp(0.25, 4.5)
            }
            ZoomSpec::FitPage => {
                let sw = (vw - SIDE_MARGIN_PX * 2.0).max(160.0) / pw;
                let sh = (vh - TOP_PAD_PX * 2.0).max(160.0) / ph;
                sw.min(sh).clamp(0.25, 4.5)
            }
        }
    }

    /// Freezes this viewport and locks `ZoomSpec::Fixed(current_scale)` so splitting a pane
    /// in half does not shrink or reflow the frozen reading view.
    pub fn freeze_with_locked_scale(&mut self, cx: &mut Context<Self>) {
        let scale = self.current_effective_scale(cx);
        self.anchor.zoom = ZoomSpec::Fixed(scale);
        self.freeze_state = FreezeState::Frozen;
        cx.update_global::<AppRegistry, _>(|reg, _| {
            reg.set_freeze_state(self.id, FreezeState::Frozen);
        });
        cx.notify();
    }

    pub fn toggle_freeze(&mut self, cx: &mut Context<Self>) {
        match self.freeze_state {
            FreezeState::Active => self.freeze_with_locked_scale(cx),
            FreezeState::Frozen => {
                self.freeze_state = FreezeState::Active;
                cx.update_global::<AppRegistry, _>(|reg, _| {
                    reg.set_freeze_state(self.id, FreezeState::Active);
                });
                cx.notify();
            }
        }
    }

    pub fn navigate_to_page_offset(
        &mut self,
        page: PageIndex,
        norm_y: f32,
        cx: &mut Context<Self>,
    ) {
        let count = self.session.read(cx).page_count();
        if count == 0 {
            return;
        }
        let clamped_page = PageIndex(page.0.min((count - 1) as u32));
        let page_h = self.session.read(cx).page_metas[clamped_page.as_usize()].norm_height;
        self.anchor.page = clamped_page;
        self.anchor.focal_norm = NormPoint::new(0.5, (norm_y / page_h.max(1.0)).clamp(0.0, 0.95));
        self.pan_x = 0.0;
        self.notify_reading_position_changed(cx);
        cx.notify();
    }

    fn notify_reading_position_changed(&self, cx: &mut Context<Self>) {
        if !self.freeze_state.is_frozen() && !self.portal_follower {
            cx.emit(ViewportEvent::ReadingPositionChanged {
                id: self.id,
                page: self.anchor.page,
                focal_y: self.anchor.focal_norm.y,
            });
        }
    }

    pub fn jump_to_page_offset_recorded(
        &mut self,
        page: PageIndex,
        norm_y: f32,
        cx: &mut Context<Self>,
    ) {
        self.jump_history.push_before_jump(self.anchor);
        self.navigate_to_page_offset(page, norm_y, cx);
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        if let Some(target) = self.jump_history.go_back(self.anchor) {
            self.anchor.page = target.page;
            self.anchor.focal_norm = target.focal_norm;
            self.pan_x = 0.0;
            self.notify_reading_position_changed(cx);
            cx.notify();
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        if let Some(target) = self.jump_history.go_forward(self.anchor) {
            self.anchor.page = target.page;
            self.anchor.focal_norm = target.focal_norm;
            self.pan_x = 0.0;
            self.notify_reading_position_changed(cx);
            cx.notify();
        }
    }

    pub fn inspect_annotation(
        &mut self,
        ann_id: AnnotationId,
        open_editor: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing_note = self
            .session
            .read(cx)
            .annotations
            .get(ann_id)
            .and_then(|a| a.note_contents.clone())
            .unwrap_or_default();
        self.inspected_annotation = Some(ann_id);
        if open_editor {
            self.editing_note = Some((ann_id, existing_note));
            window.focus(&self.note_focus_handle, cx);
        } else {
            self.editing_note = None;
        }
        cx.notify();
    }

    fn flash_frozen_guard(&mut self, cx: &mut Context<Self>) {
        self.frozen_feedback_active = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(450))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.frozen_feedback_active = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn compute_layout(
        &self,
        vw: f32,
        vh: f32,
        cx: &App,
    ) -> (f32, f32, f32, Vec<PagePlacement>) {
        let scale = self.resolve_scale(vw, vh, cx);
        let metas = &self.session.read(cx).page_metas;
        if metas.is_empty() {
            return (scale, 0.0, 0.0, Vec::new());
        }

        let mut doc_tops = Vec::with_capacity(metas.len());
        let mut cursor_y = TOP_PAD_PX;
        for (i, m) in metas.iter().enumerate() {
            doc_tops.push(cursor_y);
            cursor_y += m.norm_height * scale;
            if i + 1 < metas.len() {
                cursor_y += PAGE_GAP_PX;
            }
        }
        let total_h = cursor_y + TOP_PAD_PX;

        let anchor_idx = self.anchor.page.as_usize().min(metas.len() - 1);
        let anchor_page_h = metas[anchor_idx].norm_height * scale;
        let raw_scroll_y =
            doc_tops[anchor_idx] + self.anchor.focal_norm.y * anchor_page_h - TOP_PAD_PX;
        let max_scroll = (total_h - vh).max(0.0);
        let scroll_y = raw_scroll_y.clamp(0.0, max_scroll);

        let session_ref = self.session.read(cx);
        let fit_content = matches!(self.anchor.zoom, ZoomSpec::FitContentWidth);
        let mut placements = Vec::with_capacity(metas.len());
        for (i, m) in metas.iter().enumerate() {
            let pw = m.norm_width * scale;
            let ph = m.norm_height * scale;
            let local_y = doc_tops[i] - scroll_y;
            let base_x = if fit_content {
                if pw <= vw - SIDE_MARGIN_PX * 2.0 {
                    (vw - pw) * 0.5
                } else {
                    let (cx0, cx1) = session_ref.content_x_bounds_read(m.index);
                    let mid_norm = (cx0 + cx1) * 0.5;
                    (vw * 0.5 - mid_norm * scale).clamp(vw - pw - SIDE_MARGIN_PX, SIDE_MARGIN_PX)
                }
            } else {
                ((vw - pw) * 0.5).max(SIDE_MARGIN_PX)
            };
            let max_pan = ((pw + SIDE_MARGIN_PX * 2.0 - vw) * 0.5).max(0.0);
            let local_x = base_x + self.pan_x.clamp(-max_pan, max_pan);

            placements.push(PagePlacement {
                page: m.index,
                norm_width: m.norm_width,
                norm_height: m.norm_height,
                local_x,
                local_y,
                pixel_w: pw,
                pixel_h: ph,
            });
        }

        (scale, scroll_y, total_h, placements)
    }

    fn apply_scroll_delta(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        let bounds = self.last_canvas_bounds.get();
        let vw = bounds.size.width.as_f32().max(320.0);
        let vh = bounds.size.height.as_f32().max(240.0);
        let (scale, scroll_y, total_h, placements) = self.compute_layout(vw, vh, cx);
        if placements.is_empty() {
            return;
        }

        let max_scroll = (total_h - vh).max(0.0);
        let new_scroll_y = (scroll_y - dy).clamp(0.0, max_scroll);

        // Find page containing the top anchor line (`new_scroll_y + TOP_PAD_PX`).
        let mut cursor_y = TOP_PAD_PX;
        let mut chosen_page = placements[0].page;
        let mut chosen_focal_y = 0.0_f32;
        let target_doc_y = new_scroll_y + TOP_PAD_PX;

        for (i, p) in placements.iter().enumerate() {
            let page_bottom = cursor_y + p.pixel_h;
            if target_doc_y <= page_bottom + PAGE_GAP_PX * 0.5 || i + 1 == placements.len() {
                chosen_page = p.page;
                chosen_focal_y = ((target_doc_y - cursor_y) / p.pixel_h.max(1.0)).clamp(0.0, 0.99);
                break;
            }
            cursor_y = page_bottom + PAGE_GAP_PX;
        }

        self.anchor.page = chosen_page;
        self.anchor.focal_norm = NormPoint::new(0.5, chosen_focal_y);

        // Horizontal pan only when zoomed wider than viewport.
        let active_pw = placements[chosen_page.as_usize()].norm_width * scale;
        let max_pan = ((active_pw + SIDE_MARGIN_PX * 2.0 - vw) * 0.5).max(0.0);
        self.pan_x = (self.pan_x + dx).clamp(-max_pan, max_pan);

        self.notify_reading_position_changed(cx);
        cx.notify();
    }

    fn apply_zoom_factor(&mut self, factor: f32, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        let current = self.current_effective_scale(cx);
        let next = (current * factor).clamp(0.25, 4.5);
        self.anchor.zoom = ZoomSpec::Fixed(next);
        cx.notify();
    }

    fn step_page(&mut self, delta: i32, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        let count = self.session.read(cx).page_count();
        if count == 0 {
            return;
        }
        let next = (self.anchor.page.0 as i32 + delta).clamp(0, (count - 1) as i32) as u32;
        self.anchor.page = PageIndex(next);
        self.anchor.focal_norm = NormPoint::new(0.5, 0.0);
        self.notify_reading_position_changed(cx);
        cx.notify();
    }

    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.apply_zoom_factor(1.18, cx);
    }

    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.apply_zoom_factor(1.0 / 1.18, cx);
    }

    pub fn zoom_fit_width(&mut self, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        self.anchor.zoom = ZoomSpec::FitWidth;
        self.pan_x = 0.0;
        cx.notify();
    }

    pub fn zoom_fit_content_width(&mut self, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        let anchor_page = self.anchor.page;
        self.session.update(cx, |s, _| {
            s.content_x_bounds(anchor_page);
        });
        self.anchor.zoom = ZoomSpec::FitContentWidth;
        self.pan_x = 0.0;
        cx.notify();
    }

    pub fn next_page(&mut self, cx: &mut Context<Self>) {
        self.step_page(1, cx);
    }

    pub fn prev_page(&mut self, cx: &mut Context<Self>) {
        self.step_page(-1, cx);
    }

    pub fn toggle_area_mode(&mut self, cx: &mut Context<Self>) {
        self.area_mode = !self.area_mode;
        cx.notify();
    }

    pub fn toggle_reading_ruler(&mut self, cx: &mut Context<Self>) {
        if self.ruler_line.is_some() {
            self.ruler_line = None;
            cx.notify();
            return;
        }
        let target_page = self.anchor.page;
        let layout = self.session.update(cx, |s, _| s.ensure_text_layout(target_page));
        if layout.lines.is_empty() {
            self.ruler_line = Some((target_page, 0));
            cx.notify();
            return;
        }
        let page_h = self
            .session
            .read(cx)
            .page_metas
            .get(target_page.as_usize())
            .map(|m| m.norm_height)
            .unwrap_or(792.0);
        let desired_y = (self.anchor.focal_norm.y + 0.18).min(0.95) * page_h;
        let best_idx = layout
            .lines
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (a.bounds.center().y - desired_y)
                    .abs()
                    .total_cmp(&(b.bounds.center().y - desired_y).abs())
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.ruler_line = Some((target_page, best_idx));
        cx.notify();
    }

    fn set_ruler_at_point(&mut self, page: PageIndex, norm_pt: NormPoint, cx: &mut Context<Self>) {
        let layout = self.session.update(cx, |s, _| s.ensure_text_layout(page));
        if layout.lines.is_empty() {
            self.ruler_line = Some((page, 0));
            cx.notify();
            return;
        }
        let idx = layout
            .lines
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = (a.bounds.center().y - norm_pt.y).abs()
                    + (a.bounds.center().x - norm_pt.x).abs() * 0.15;
                let db = (b.bounds.center().y - norm_pt.y).abs()
                    + (b.bounds.center().x - norm_pt.x).abs() * 0.15;
                da.total_cmp(&db)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.ruler_line = Some((page, idx));
        cx.notify();
    }

    fn step_reading_ruler(&mut self, delta: i32, cx: &mut Context<Self>) -> bool {
        let Some((cur_page, cur_idx)) = self.ruler_line else {
            return false;
        };
        let page_count = self.session.read(cx).page_count();
        if page_count == 0 {
            return false;
        }

        let cur_layout = self.session.update(cx, |s, _| s.ensure_text_layout(cur_page));
        let (next_page, next_idx) = if delta > 0 {
            if cur_idx + 1 < cur_layout.lines.len() {
                (cur_page, cur_idx + 1)
            } else {
                let mut np = cur_page.0 + 1;
                let mut found = (cur_page, cur_idx);
                while (np as usize) < page_count {
                    let nl = self
                        .session
                        .update(cx, |s, _| s.ensure_text_layout(PageIndex(np)));
                    if !nl.lines.is_empty() {
                        found = (PageIndex(np), 0);
                        break;
                    }
                    np += 1;
                }
                found
            }
        } else if cur_idx > 0 {
            (cur_page, cur_idx - 1)
        } else {
            let mut np = cur_page.0 as i32 - 1;
            let mut found = (cur_page, 0);
            while np >= 0 {
                let nl = self
                    .session
                    .update(cx, |s, _| s.ensure_text_layout(PageIndex(np as u32)));
                if !nl.lines.is_empty() {
                    found = (PageIndex(np as u32), nl.lines.len() - 1);
                    break;
                }
                np -= 1;
            }
            found
        };

        self.ruler_line = Some((next_page, next_idx));

        // Keep the active reading line comfortably inside the [18%..80%] vertical band.
        let bounds = self.last_canvas_bounds.get();
        let vw = bounds.size.width.as_f32().max(320.0);
        let vh = bounds.size.height.as_f32().max(240.0);
        let line_y = self
            .session
            .update(cx, |s, _| s.ensure_text_layout(next_page))
            .lines
            .get(next_idx)
            .map(|l| l.bounds.center().y)
            .unwrap_or(0.0);
        let (scale, _, _, placements) = self.compute_layout(vw, vh, cx);
        if let Some(p) = placements.iter().find(|p| p.page == next_page) {
            let screen_y = p.local_y + line_y * scale;
            if screen_y < vh * 0.18 || screen_y > vh * 0.80 {
                let desired_top_norm_y = (line_y - (vh * 0.35) / scale.max(0.1)).max(0.0);
                self.navigate_to_page_offset(next_page, desired_top_norm_y, cx);
                return true;
            }
        }
        cx.notify();
        true
    }

    pub fn scroll_line_down(&mut self, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        if !self.step_reading_ruler(1, cx) {
            self.apply_scroll_delta(0.0, -64.0, cx);
        }
    }

    pub fn scroll_line_up(&mut self, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        if !self.step_reading_ruler(-1, cx) {
            self.apply_scroll_delta(0.0, 64.0, cx);
        }
    }

    pub fn scroll_half_page_down(&mut self, cx: &mut Context<Self>) {
        let vh = self.last_canvas_bounds.get().size.height.as_f32().max(320.0);
        self.apply_scroll_delta(0.0, -vh * 0.48, cx);
    }

    pub fn scroll_half_page_up(&mut self, cx: &mut Context<Self>) {
        let vh = self.last_canvas_bounds.get().size.height.as_f32().max(320.0);
        self.apply_scroll_delta(0.0, vh * 0.48, cx);
    }

    pub fn scroll_screen_down(&mut self, cx: &mut Context<Self>) {
        self.page_count_prefix.clear();
        let vh = self.last_canvas_bounds.get().size.height.as_f32().max(320.0);
        let step = (vh - 48.0).max(vh * 0.85);
        self.apply_scroll_delta(0.0, -step, cx);
    }

    pub fn scroll_screen_up(&mut self, cx: &mut Context<Self>) {
        self.page_count_prefix.clear();
        let vh = self.last_canvas_bounds.get().size.height.as_f32().max(320.0);
        let step = (vh - 48.0).max(vh * 0.85);
        self.apply_scroll_delta(0.0, step, cx);
    }

    fn try_consume_page_prefix_jump(&mut self, cx: &mut Context<Self>) -> bool {
        if self.page_count_prefix.is_empty() {
            return false;
        }
        let raw = std::mem::take(&mut self.page_count_prefix);
        let Ok(page_num) = raw.parse::<u32>() else {
            cx.notify();
            return true;
        };
        if page_num == 0 {
            cx.notify();
            return true;
        }
        let last = self.session.read(cx).page_count().saturating_sub(1) as u32;
        let target = PageIndex(page_num.saturating_sub(1).min(last));
        if self.freeze_state.is_frozen() {
            cx.emit(ViewportEvent::RequestSmartLinkRoute {
                source_id: self.id,
                page: target,
                top_norm_y: Some(0.0),
            });
        } else {
            self.jump_to_page_offset_recorded(target, 0.0, cx);
        }
        cx.notify();
        true
    }

    pub fn go_first_page(&mut self, cx: &mut Context<Self>) {
        if self.try_consume_page_prefix_jump(cx) {
            return;
        }
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        self.jump_to_page_offset_recorded(PageIndex(0), 0.0, cx);
    }

    pub fn go_last_page(&mut self, cx: &mut Context<Self>) {
        if self.try_consume_page_prefix_jump(cx) {
            return;
        }
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        let last = self.session.read(cx).page_count().saturating_sub(1) as u32;
        self.jump_to_page_offset_recorded(PageIndex(last), 0.0, cx);
    }

    pub fn begin_set_mark(&mut self, cx: &mut Context<Self>) {
        self.begin_mark_op(PendingMarkOp::Set, cx);
    }

    pub fn begin_jump_mark(&mut self, cx: &mut Context<Self>) {
        self.begin_mark_op(PendingMarkOp::Jump, cx);
    }

    pub fn begin_typed_highlight(&mut self, cx: &mut Context<Self>) {
        if self.selection.is_none() && self.inspected_annotation.is_none() {
            self.session.update(cx, |s, cx| {
                s.status_message = Some((
                    "Select text or click an annotation first, then press H + [a-z]".to_string(),
                    false,
                ));
                cx.notify();
            });
            return;
        }
        self.begin_mark_op(PendingMarkOp::TypedHighlight, cx);
    }

    fn begin_mark_op(&mut self, op: PendingMarkOp, cx: &mut Context<Self>) {
        self.hint_mode = None;
        self.pending_mark_op = Some(op);
        cx.notify();
    }

    fn overview_jump(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(ov) = self.overview_preview.take() else {
            return false;
        };
        self.follow_internal_target(ov.page, ov.top_norm_y, cx);
        true
    }

    fn overview_split(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(ov) = self.overview_preview.take() else {
            return false;
        };
        if !self.freeze_state.is_frozen() {
            self.freeze_with_locked_scale(cx);
        }
        cx.emit(ViewportEvent::RequestSmartLinkRoute {
            source_id: self.id,
            page: ov.page,
            top_norm_y: Some(ov.top_norm_y),
        });
        cx.notify();
        true
    }

    fn close_overview(&mut self, cx: &mut Context<Self>) -> bool {
        if self.overview_preview.take().is_some() {
            cx.notify();
            true
        } else {
            false
        }
    }

    pub fn link_portal(&mut self, cx: &mut Context<Self>) {
        if let Some(ov) = self.overview_preview.take() {
            let page_h = self
                .session
                .read(cx)
                .page_metas
                .get(ov.page.as_usize())
                .map(|m| m.norm_height)
                .unwrap_or(792.0);
            let dst = ReadingAnchor {
                page: ov.page,
                focal_norm: NormPoint::new(0.5, (ov.top_norm_y / page_h.max(1.0)).clamp(0.0, 0.95)),
                zoom: ZoomSpec::FitWidth,
            };
            let src_page = self.anchor.page;
            let src_y = self.anchor.focal_norm.y;
            self.session.update(cx, |s, cx| {
                s.add_portal(src_page, src_y, dst, ov.label, cx);
            });
            cx.emit(ViewportEvent::RequestEnsurePortalFollower {
                source_id: self.id,
                dst,
            });
            cx.notify();
            return;
        }

        let pending_src = self.session.read(cx).pending_portal_src;
        match pending_src {
            None => {
                let src = (self.anchor.page, self.anchor.focal_norm.y);
                self.session.update(cx, |s, cx| {
                    s.pending_portal_src = Some(src);
                    s.status_message = Some((
                        format!(
                            "Portal source armed at p.{} — scroll to figure/table and press P to link",
                            src.0.one_based()
                        ),
                        false,
                    ));
                    cx.notify();
                });
            }
            Some((src_page, src_y)) => {
                let dst = self.anchor;
                self.session.update(cx, |s, cx| {
                    let label = format!("p.{} ref", dst.page.one_based());
                    s.add_portal(src_page, src_y, dst, label, cx);
                });
                let src_page_h = self
                    .session
                    .read(cx)
                    .page_metas
                    .get(src_page.as_usize())
                    .map(|m| m.norm_height)
                    .unwrap_or(792.0);
                self.navigate_to_page_offset(src_page, src_y * src_page_h, cx);
                cx.emit(ViewportEvent::RequestEnsurePortalFollower {
                    source_id: self.id,
                    dst,
                });
            }
        }
    }

    pub fn delete_nearest_portal(&mut self, cx: &mut Context<Self>) {
        let p = self.anchor.page;
        let fy = self.anchor.focal_norm.y;
        self.session.update(cx, |s, cx| {
            s.delete_nearest_portal(p, fy, cx);
        });
    }

    pub fn toggle_auto_scroll(&mut self, cx: &mut Context<Self>) {
        if self.freeze_state.is_frozen() {
            self.flash_frozen_guard(cx);
            return;
        }
        if self.auto_scroll_speed.is_some() {
            self.auto_scroll_speed = None;
            self.session.update(cx, |s, cx| {
                s.status_message = Some(("Auto-scroll stopped".to_string(), false));
                cx.notify();
            });
            cx.notify();
            return;
        }

        let speed = 56.0_f32;
        self.auto_scroll_speed = Some(speed);
        self.session.update(cx, |s, cx| {
            s.status_message = Some((
                format!("Auto-scroll ON ({speed:.0} px/s — Alt+=/Alt+- speed, F6 stop)"),
                false,
            ));
            cx.notify();
        });
        self.ensure_auto_scroll_loop(cx);
        cx.notify();
    }

    pub fn adjust_auto_scroll_speed(&mut self, delta: f32, cx: &mut Context<Self>) {
        let current = self.auto_scroll_speed.unwrap_or(48.0);
        let next = (current + delta).clamp(16.0, 420.0);
        self.auto_scroll_speed = Some(next);
        self.session.update(cx, |s, cx| {
            s.status_message = Some((format!("Auto-scroll speed: {next:.0} px/s"), false));
            cx.notify();
        });
        self.ensure_auto_scroll_loop(cx);
        cx.notify();
    }

    fn ensure_auto_scroll_loop(&mut self, cx: &mut Context<Self>) {
        if self.auto_scroll_loop_active {
            return;
        }
        self.auto_scroll_loop_active = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let keep_going = this
                    .update(cx, |vp, cx| {
                        let Some(speed) = vp.auto_scroll_speed else {
                            vp.auto_scroll_loop_active = false;
                            return false;
                        };
                        if vp.freeze_state.is_frozen() {
                            vp.auto_scroll_speed = None;
                            vp.auto_scroll_loop_active = false;
                            return false;
                        }
                        vp.apply_scroll_delta(0.0, -(speed * 0.016), cx);
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn trigger_synctex_inverse_at(
        &mut self,
        page: PageIndex,
        pt: NormPoint,
        cx: &mut Context<Self>,
    ) {
        let synctex_opt = self.session.read(cx).synctex.clone();
        let Some(synctex) = synctex_opt else {
            self.session.update(cx, |s, cx| {
                s.status_message = Some(("No .synctex.gz file found next to PDF".to_string(), true));
                cx.notify();
            });
            return;
        };
        let Some(loc) = synctex.inverse_search(page, pt) else {
            self.session.update(cx, |s, cx| {
                s.status_message = Some(("No SyncTeX source match near cursor".to_string(), true));
                cx.notify();
            });
            return;
        };
        let custom_cmd = cx.global::<AppRegistry>().user_config.inverse_search_command.clone();
        let (msg, is_err) = run_synctex_inverse_editor(&loc, custom_cmd.as_deref());
        self.session.update(cx, |s, cx| {
            s.status_message = Some((msg, is_err));
            cx.notify();
        });
    }

    pub fn synctex_inverse_here(&mut self, cx: &mut Context<Self>) {
        let (page, pt) = if let Some((rp, ridx)) = self.ruler_line {
            let layout = self.session.update(cx, |s, _| s.ensure_text_layout(rp));
            let center = layout
                .lines
                .get(ridx)
                .map(|l| l.bounds.center())
                .unwrap_or_else(|| NormPoint::new(200.0, 200.0));
            (rp, center)
        } else {
            let meta = self
                .session
                .read(cx)
                .page_metas
                .get(self.anchor.page.as_usize())
                .cloned();
            let (pw, ph) = meta
                .map(|m| (m.norm_width, m.norm_height))
                .unwrap_or((612.0, 792.0));
            let y = ((self.anchor.focal_norm.y + 0.15).clamp(0.0, 0.95)) * ph;
            (self.anchor.page, NormPoint::new(pw * 0.35, y))
        };
        self.trigger_synctex_inverse_at(page, pt, cx);
    }

    pub fn current_extension_context(&mut self, cx: &mut App) -> ExtensionContext {
        let selected_text = self.selected_text_string(cx);
        let page = self.anchor.page;
        let line_text = if let Some((rp, ridx)) = self.ruler_line {
            let layout = self.session.update(cx, |s, _| s.ensure_text_layout(rp));
            layout
                .lines
                .get(ridx)
                .and_then(|l| {
                    let first = layout.glyphs.get(l.span_range.start)?;
                    let last = layout.glyphs.get(l.span_range.end.checked_sub(1)?)?;
                    layout
                        .full_text
                        .get(first.byte_range.start..last.byte_range.end)
                        .map(str::to_string)
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        let file_path = self
            .session
            .read(cx)
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let doi = extract_first_doi(&selected_text);
        ExtensionContext {
            file_path,
            page_one_based: page.one_based(),
            selected_text,
            line_text,
            doi,
        }
    }

    pub fn begin_hint_mode(&mut self, kind: HintModeKind, cx: &mut Context<Self>) {
        self.pending_mark_op = None;
        let bounds = self.last_canvas_bounds.get();
        let vw = bounds.size.width.as_f32().max(320.0);
        let vh = bounds.size.height.as_f32().max(240.0);
        let (scale, _, _, placements) = self.compute_layout(vw, vh, cx);
        let visible: Vec<PagePlacement> = placements
            .into_iter()
            .filter(|p| p.local_y + p.pixel_h >= 0.0 && p.local_y <= vh)
            .collect();

        let layouts = if matches!(kind, HintModeKind::Jump | HintModeKind::Overview) {
            Some(self.session.update(cx, |s, _| s.ensure_all_layouts_now()))
        } else {
            None
        };

        let mut raw_items: Vec<(PageIndex, NormRect, HintAction)> = Vec::new();
        for p in &visible {
            let vis_min_y = ((-p.local_y) / scale).clamp(0.0, p.norm_height);
            let vis_max_y = ((vh - p.local_y) / scale).clamp(0.0, p.norm_height);

            match kind {
                HintModeKind::Jump | HintModeKind::Overview => {
                    let links = self.session.read(cx).links_for_page(p.page).to_vec();
                    for link in links {
                        if link.rect.max_y < vis_min_y || link.rect.min_y > vis_max_y {
                            continue;
                        }
                        let action = match link.target {
                            LinkTarget::InternalPage { page, top_norm_y } => {
                                HintAction::InternalJump {
                                    page,
                                    top_norm_y: top_norm_y.unwrap_or(0.0),
                                    label: format!("Link -> p.{}", page.one_based()),
                                }
                            }
                            LinkTarget::ExternalUrl(url) => HintAction::ExternalUrl(url),
                        };
                        raw_items.push((p.page, link.rect, action));
                    }
                    if let Some(ref all_layouts) = layouts {
                        for r in collect_page_smart_refs(p.page, vis_min_y, vis_max_y, all_layouts)
                        {
                            let dup = raw_items
                                .iter()
                                .any(|(ep, er, _)| *ep == p.page && er.intersects(r.rect));
                            if !dup {
                                raw_items.push((
                                    p.page,
                                    r.rect,
                                    HintAction::InternalJump {
                                        page: r.target.page,
                                        top_norm_y: (r.target.top_norm_y - 18.0).max(0.0),
                                        label: r.target.label,
                                    },
                                ));
                            }
                        }
                    }
                }
                HintModeKind::SelectStart | HintModeKind::SelectEnd { .. } => {
                    let layout = self.session.update(cx, |s, _| s.ensure_text_layout(p.page));
                    for line in &layout.lines {
                        if line.bounds.max_y < vis_min_y || line.bounds.min_y > vis_max_y {
                            continue;
                        }
                        let mut span_idx = line.span_range.start;
                        while span_idx < line.span_range.end {
                            while span_idx < line.span_range.end
                                && layout.glyphs[span_idx].is_synthetic_space
                            {
                                span_idx += 1;
                            }
                            let word_start = span_idx;
                            while span_idx < line.span_range.end {
                                let g = &layout.glyphs[span_idx];
                                let slice = &layout.full_text[g.byte_range.clone()];
                                if g.is_synthetic_space || slice.chars().all(char::is_whitespace) {
                                    break;
                                }
                                span_idx += 1;
                            }
                            let word_end = span_idx;
                            if word_end > word_start {
                                let chosen_idx = match kind {
                                    HintModeKind::SelectEnd { .. } => word_end - 1,
                                    _ => word_start,
                                };
                                let rect = layout.glyphs[word_start].rect;
                                raw_items.push((
                                    p.page,
                                    rect,
                                    HintAction::SelectChar(PageCharPos::new(p.page, chosen_idx)),
                                ));
                                if raw_items.len() >= 600 {
                                    break;
                                }
                            }
                        }
                        if raw_items.len() >= 600 {
                            break;
                        }
                    }
                }
            }
        }

        if raw_items.is_empty() {
            self.hint_mode = None;
            self.session.update(cx, |s, cx| {
                s.status_message = Some(("No visible hint targets in viewport".to_string(), false));
                cx.notify();
            });
            return;
        }

        raw_items.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| (a.1.min_y / 8.0).round().total_cmp(&(b.1.min_y / 8.0).round()))
                .then_with(|| a.1.min_x.total_cmp(&b.1.min_x))
        });
        raw_items.truncate(HINT_ALPHABET.len() * HINT_ALPHABET.len());
        let labels = generate_hint_labels(raw_items.len());
        let candidates = raw_items
            .into_iter()
            .zip(labels)
            .map(|((page, rect, action), label)| HintCandidate {
                label,
                page,
                rect,
                action,
            })
            .collect();

        self.hint_mode = Some(HintModeState {
            kind,
            typed: String::new(),
            candidates,
        });
        cx.notify();
    }

    fn execute_hint_candidate(
        &mut self,
        kind: HintModeKind,
        cand: HintCandidate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hint_mode = None;
        match kind {
            HintModeKind::Jump => match cand.action {
                HintAction::InternalJump {
                    page, top_norm_y, ..
                } => {
                    self.overview_preview = None;
                    self.follow_internal_target(page, top_norm_y, cx);
                }
                HintAction::ExternalUrl(url) => {
                    let opened = crate::external::open_external_url(&url);
                    self.session.update(cx, |s, cx| {
                        s.status_message = Some((
                            if opened {
                                format!("Opened {url}")
                            } else {
                                format!("External URL: {url}")
                            },
                            !opened,
                        ));
                        cx.notify();
                    });
                }
                HintAction::SelectChar(_) => {}
            },
            HintModeKind::Overview => match cand.action {
                HintAction::InternalJump {
                    page,
                    top_norm_y,
                    label,
                } => {
                    let bounds = self.last_canvas_bounds.get();
                    let vw = bounds.size.width.as_f32().max(320.0);
                    let vh = bounds.size.height.as_f32().max(240.0);
                    let (scale, _, _, placements) = self.compute_layout(vw, vh, cx);
                    let anchor_pt = placements
                        .iter()
                        .find(|p| p.page == cand.page)
                        .map(|p| {
                            point(
                                bounds.origin.x + px(p.local_x + cand.rect.min_x * scale),
                                bounds.origin.y + px(p.local_y + cand.rect.min_y * scale),
                            )
                        })
                        .unwrap_or_else(|| point(bounds.origin.x + px(40.), bounds.origin.y + px(60.)));
                    self.overview_preview = Some(OverviewPreview {
                        label,
                        page,
                        top_norm_y,
                        anchor_win_pt: anchor_pt,
                    });
                }
                HintAction::ExternalUrl(url) => {
                    let _ = crate::external::open_external_url(&url);
                }
                HintAction::SelectChar(_) => {}
            },
            HintModeKind::SelectStart => {
                if let HintAction::SelectChar(start) = cand.action {
                    self.begin_hint_mode(HintModeKind::SelectEnd { start }, cx);
                }
            }
            HintModeKind::SelectEnd { start } => {
                if let HintAction::SelectChar(end) = cand.action {
                    self.selection = Some(TextSelectionRange::new(start, end));
                    self.inspected_annotation = None;
                    self.editing_note = None;
                    window.focus(&self.focus_handle, cx);
                }
            }
        }
        cx.notify();
    }

    fn handle_viewport_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;

        if self.hint_mode.is_some() {
            cx.stop_propagation();
            if ks.key == "escape" {
                self.hint_mode = None;
                cx.notify();
                return;
            }
            if ks.key == "backspace" {
                if let Some(ref mut hm) = self.hint_mode {
                    if !hm.typed.is_empty() {
                        hm.typed.pop();
                    } else {
                        self.hint_mode = None;
                    }
                    cx.notify();
                }
                return;
            }
            if !ks.modifiers.platform
                && !ks.modifiers.control
                && !ks.modifiers.alt
                && let Some(ch_str) = &ks.key_char
                && let Some(ch) = ch_str.chars().next()
                && ch.is_ascii_alphabetic()
            {
                let lower = ch.to_ascii_lowercase();
                let mut matched_exec: Option<(HintModeKind, HintCandidate)> = None;
                let mut clear_hints = false;
                if let Some(ref mut hm) = self.hint_mode {
                    hm.typed.push(lower);
                    let remaining: Vec<&HintCandidate> = hm
                        .candidates
                        .iter()
                        .filter(|c| c.label.starts_with(&hm.typed))
                        .collect();
                    if remaining.is_empty() {
                        clear_hints = true;
                    } else if remaining.len() == 1 || remaining.iter().any(|c| c.label == hm.typed) {
                        let chosen = remaining
                            .iter()
                            .find(|c| c.label == hm.typed)
                            .copied()
                            .unwrap_or(remaining[0])
                            .clone();
                        matched_exec = Some((hm.kind, chosen));
                    }
                }
                if clear_hints {
                    self.hint_mode = None;
                    cx.notify();
                } else if let Some((kind, chosen)) = matched_exec {
                    self.execute_hint_candidate(kind, chosen, window, cx);
                } else {
                    cx.notify();
                }
            }
            return;
        }

        if self.pending_mark_op.is_none() && self.editing_note.is_none() {
            if !self.page_count_prefix.is_empty() && ks.key == "escape" {
                self.page_count_prefix.clear();
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if !self.page_count_prefix.is_empty() && ks.key == "backspace" {
                self.page_count_prefix.pop();
                cx.stop_propagation();
                cx.notify();
                return;
            }
            if !ks.modifiers.platform
                && !ks.modifiers.control
                && !ks.modifiers.alt
                && let Some(ch_str) = &ks.key_char
                && let Some(ch) = ch_str.chars().next()
                && ch.is_ascii_digit()
                && (!self.page_count_prefix.is_empty() || ch != '0')
            {
                if self.page_count_prefix.len() < 5 {
                    self.page_count_prefix.push(ch);
                    cx.notify();
                }
                cx.stop_propagation();
                return;
            }
            if !self.page_count_prefix.is_empty()
                && ks.key != "g"
                && !matches!(ks.key.as_str(), "shift" | "control" | "alt" | "platform")
            {
                self.page_count_prefix.clear();
                cx.notify();
            }

            if self.overview_preview.is_some() {
                if ks.key == "enter"
                    && !ks.modifiers.platform
                    && !ks.modifiers.control
                    && !ks.modifiers.shift
                    && !ks.modifiers.alt
                {
                    self.page_count_prefix.clear();
                    self.overview_jump(cx);
                    cx.stop_propagation();
                    return;
                }
                if ((ks.modifiers.platform || ks.modifiers.control || ks.modifiers.shift)
                    && ks.key == "enter")
                    || (!ks.modifiers.platform
                        && !ks.modifiers.control
                        && !ks.modifiers.alt
                        && self.selection.is_none()
                        && ks
                            .key_char
                            .as_deref()
                            .is_some_and(|c| c.eq_ignore_ascii_case("s")))
                {
                    self.page_count_prefix.clear();
                    self.overview_split(cx);
                    cx.stop_propagation();
                    return;
                }
                if ks.key == "escape"
                    || (!ks.modifiers.platform
                        && !ks.modifiers.control
                        && !ks.modifiers.alt
                        && ks
                            .key_char
                            .as_deref()
                            .is_some_and(|c| c.eq_ignore_ascii_case("x")))
                {
                    self.page_count_prefix.clear();
                    self.close_overview(cx);
                    cx.stop_propagation();
                    return;
                }
            }
        }

        let Some(op) = self.pending_mark_op else {
            return;
        };
        cx.stop_propagation();
        if ks.key == "escape" {
            self.pending_mark_op = None;
            cx.notify();
            return;
        }
        if !ks.modifiers.platform
            && !ks.modifiers.control
            && let Some(ch_str) = &ks.key_char
            && let Some(ch) = ch_str.chars().next()
            && ch.is_ascii_alphanumeric()
        {
            self.pending_mark_op = None;
            match op {
                PendingMarkOp::TypedHighlight => {
                    if let Some(cat) = normalize_category_char(ch) {
                        let (cat_label, color) =
                            cx.global::<AppRegistry>().resolve_highlight_category(cat);
                        if self.selection.is_some() {
                            self.commit_text_annotation(
                                AnnotationKind::Highlight,
                                Some(cat),
                                color,
                                false,
                                window,
                                cx,
                            );
                        } else if let Some(ann_id) = self.inspected_annotation {
                            self.session.update(cx, |s, cx| {
                                s.update_annotation_category(ann_id, Some(cat), color, cx);
                            });
                        }
                        self.session.update(cx, |s, cx| {
                            s.status_message = Some((format!("Category [{cat}: {cat_label}]"), false));
                            cx.notify();
                        });
                    }
                }
                PendingMarkOp::Set => {
                    let anchor = self.anchor;
                    self.session.update(cx, |s, cx| {
                        s.marks.insert(ch, anchor);
                        s.status_message = Some((
                            format!("Mark '{ch}' set (p.{})", anchor.page.one_based()),
                            false,
                        ));
                        cx.notify();
                    });
                }
                PendingMarkOp::Jump => {
                    let target_opt = self.session.read(cx).marks.get(&ch).copied();
                    if let Some(target) = target_opt {
                        let page_h = self
                            .session
                            .read(cx)
                            .page_metas
                            .get(target.page.as_usize())
                            .map(|m| m.norm_height)
                            .unwrap_or(792.0);
                        let top_norm_y = target.focal_norm.y * page_h;
                        if self.freeze_state.is_frozen() {
                            cx.emit(ViewportEvent::RequestSmartLinkRoute {
                                source_id: self.id,
                                page: target.page,
                                top_norm_y: Some(top_norm_y),
                            });
                        } else {
                            self.jump_to_page_offset_recorded(target.page, top_norm_y, cx);
                        }
                    } else {
                        self.session.update(cx, |s, cx| {
                            s.status_message = Some((format!("Mark '{ch}' not set"), true));
                            cx.notify();
                        });
                    }
                }
            }
            cx.notify();
        }
    }

    fn window_pos_to_page(
        &self,
        win_pos: Point<Pixels>,
        cx: &App,
    ) -> Option<(PageIndex, NormPoint, f32)> {
        let bounds = self.last_canvas_bounds.get();
        let vw = bounds.size.width.as_f32();
        let vh = bounds.size.height.as_f32();
        if vw <= 1.0 || vh <= 1.0 {
            return None;
        }
        let local_x = win_pos.x.as_f32() - bounds.origin.x.as_f32();
        let local_y = win_pos.y.as_f32() - bounds.origin.y.as_f32();
        let (scale, _, _, placements) = self.compute_layout(vw, vh, cx);

        // Prefer direct page hit, otherwise snap to nearest vertical page for smooth dragging.
        let mut best: Option<(f32, &PagePlacement)> = None;
        for p in &placements {
            let dy = if local_y < p.local_y {
                p.local_y - local_y
            } else if local_y > p.local_y + p.pixel_h {
                local_y - (p.local_y + p.pixel_h)
            } else {
                0.0
            };
            if dy == 0.0 {
                best = Some((0.0, p));
                break;
            }
            if best.is_none_or(|(d, _)| dy < d) {
                best = Some((dy, p));
            }
        }
        let (_, p) = best?;
        let nx = ((local_x - p.local_x) / scale).clamp(0.0, p.norm_width);
        let ny = ((local_y - p.local_y) / scale).clamp(0.0, p.norm_height);
        Some((p.page, NormPoint::new(nx, ny), scale))
    }

    fn selected_text_string(&self, cx: &mut App) -> String {
        let Some(sel) = self.selection else {
            return String::new();
        };
        let (start, end) = sel.ordered();
        let mut out = String::new();
        for p_raw in start.page.0..=end.page.0 {
            let page = PageIndex(p_raw);
            let layout = self
                .session
                .update(cx, |s, _| s.ensure_text_layout(page));
            if layout.is_empty() {
                continue;
            }
            let s_idx = if page == start.page {
                start.glyph_index
            } else {
                0
            };
            let e_idx = if page == end.page {
                end.glyph_index
            } else {
                layout.glyphs.len() - 1
            };
            let slice = layout.slice_text(s_idx, e_idx);
            if !slice.is_empty() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&slice);
            }
        }
        out
    }

    fn selection_quads_by_page(&self, cx: &mut App) -> Vec<(PageIndex, Vec<NormQuad>)> {
        let Some(sel) = self.selection else {
            return Vec::new();
        };
        let (start, end) = sel.ordered();
        let mut result = Vec::new();
        for p_raw in start.page.0..=end.page.0 {
            let page = PageIndex(p_raw);
            let layout = self
                .session
                .update(cx, |s, _| s.ensure_text_layout(page));
            if layout.is_empty() {
                continue;
            }
            let s_idx = if page == start.page {
                start.glyph_index
            } else {
                0
            };
            let e_idx = if page == end.page {
                end.glyph_index
            } else {
                layout.glyphs.len() - 1
            };
            let quads = layout.selection_quads(s_idx, e_idx);
            if !quads.is_empty() {
                result.push((page, quads));
            }
        }
        result
    }

    pub fn copy_selection(&self, cx: &mut Context<Self>) {
        let text = self.selected_text_string(cx);
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub fn lookup_selection_scholar(&self, cx: &mut Context<Self>) {
        let text = self.selected_text_string(cx);
        let Some(url) = crate::external::build_scholar_or_doi_url(&text) else {
            return;
        };
        let opened = crate::external::open_external_url(&url);
        self.session.update(cx, |s, cx| {
            s.status_message = Some((
                if opened {
                    format!("Opened Scholar/DOI lookup: {url}")
                } else {
                    format!("Could not launch browser for {url}")
                },
                !opened,
            ));
            cx.notify();
        });
    }

    pub fn highlight_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text_annotation(
            AnnotationKind::Highlight,
            None,
            RgbaColor::YELLOW,
            false,
            window,
            cx,
        );
    }

    pub fn underline_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text_annotation(
            AnnotationKind::Underline,
            None,
            RgbaColor::BLUE,
            false,
            window,
            cx,
        );
    }

    pub fn strikeout_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text_annotation(
            AnnotationKind::StrikeOut,
            None,
            RgbaColor::PINK,
            false,
            window,
            cx,
        );
    }

    pub fn annotate_selection_with_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_text_annotation(
            AnnotationKind::Highlight,
            None,
            RgbaColor::YELLOW,
            true,
            window,
            cx,
        );
    }

    fn commit_text_annotation(
        &mut self,
        kind: AnnotationKind,
        category: Option<char>,
        color: RgbaColor,
        open_note_editor: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let per_page_quads = self.selection_quads_by_page(cx);
        if per_page_quads.is_empty() {
            return;
        }
        let full_quote = self.selected_text_string(cx);
        let mut first_created: Option<AnnotationId> = None;

        for (page, quads) in per_page_quads {
            let bounds = quads
                .iter()
                .map(NormQuad::bounding_rect)
                .reduce(NormRect::union)
                .unwrap_or_else(|| NormRect::new(0.0, 0.0, 10.0, 10.0));
            let id = AnnotationId::new();
            if first_created.is_none() {
                first_created = Some(id);
            }
            let ann = Annotation {
                id,
                pdf_object_id: None,
                page,
                kind,
                quads,
                bounds,
                color,
                category,
                selected_text: Some(full_quote.clone()),
                note_contents: None,
                modified_at: pdf_now_string(),
            };
            self.session.update(cx, |s, cx| s.add_annotation(ann, cx));
        }

        self.selection = None;
        if let Some(id) = first_created {
            self.inspect_annotation(id, open_note_editor, window, cx);
        }
        cx.notify();
    }

    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        window.focus(&self.focus_handle, cx);
        cx.emit(ViewportEvent::Focused(self.id));

        let Some((page, norm_pt, _)) = self.window_pos_to_page(event.position, cx) else {
            return;
        };

        // SyncTeX inverse jump via Cmd+Click (or Ctrl+Click) when .synctex is present.
        if (event.modifiers.platform || event.modifiers.control)
            && self.session.read(cx).synctex.is_some()
        {
            self.trigger_synctex_inverse_at(page, norm_pt, cx);
            return;
        }

        // Area Note marquee mode (Alt+Drag or Area button toggled).
        if self.area_mode || event.modifiers.alt {
            self.selection = None;
            self.inspected_annotation = None;
            self.editing_note = None;
            self.drag_mode = DragMode::AreaMarquee {
                page,
                start: norm_pt,
                current: norm_pt,
            };
            cx.notify();
            return;
        }

        // Handle double-click (word) and triple-click (line) immediately.
        if event.click_count >= 2 {
            let layout = self.session.update(cx, |s, _| s.ensure_text_layout(page));
            if let Some(glyph_idx) = layout.hit_test(norm_pt) {
                let range = if event.click_count == 2 {
                    layout.word_range_at(glyph_idx)
                } else {
                    layout.line_range_at(glyph_idx)
                };
                if let Some((start, end)) = range {
                    self.selection = Some(TextSelectionRange::new(
                        PageCharPos::new(page, start),
                        PageCharPos::new(page, end),
                    ));
                    self.inspected_annotation = None;
                    self.editing_note = None;
                    self.drag_mode = DragMode::None;
                    cx.notify();
                    return;
                }
            }
        }

        let layout = self.session.update(cx, |s, _| s.ensure_text_layout(page));
        if let Some(glyph_idx) = layout.hit_test(norm_pt) {
            let pos = PageCharPos::new(page, glyph_idx);
            self.drag_mode = DragMode::TextSelect { anchor: pos };
            self.selection = Some(TextSelectionRange::new(pos, pos));
        } else {
            self.drag_mode = DragMode::None;
            self.selection = None;
        }
        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((page, norm_pt, _)) = self.window_pos_to_page(event.position, cx) else {
            return;
        };

        match &mut self.drag_mode {
            DragMode::None => {
                let hovered = self
                    .session
                    .read(cx)
                    .links_for_page(page)
                    .iter()
                    .position(|l| l.rect.inflate(2.0).contains(norm_pt))
                    .map(|idx| (page, idx));
                if hovered != self.hovered_link {
                    self.hovered_link = hovered;
                    cx.notify();
                }
            }
            DragMode::TextSelect { anchor } => {
                let anchor = *anchor;
                let layout = self.session.update(cx, |s, _| s.ensure_text_layout(page));
                if let Some(glyph_idx) = layout.hit_test(norm_pt) {
                    let focus = PageCharPos::new(page, glyph_idx);
                    self.selection = Some(TextSelectionRange::new(anchor, focus));
                    cx.notify();
                }
            }
            DragMode::AreaMarquee {
                page: marquee_page,
                start,
                current,
            } => {
                if *marquee_page == page {
                    *current = norm_pt;
                } else if norm_pt.y >= 0.0 {
                    // Clamp to page edge when dragging slightly beyond top/bottom.
                    let mh = self.session.read(cx).page_metas[marquee_page.as_usize()].norm_height;
                    let mw = self.session.read(cx).page_metas[marquee_page.as_usize()].norm_width;
                    let cy = if page < *marquee_page { 0.0 } else { mh };
                    *current = NormPoint::new(norm_pt.x.clamp(0.0, mw), cy);
                    let _ = start;
                }
                cx.notify();
            }
        }
    }

    fn resolve_target_at_point(
        &mut self,
        page: PageIndex,
        norm_pt: NormPoint,
        cx: &mut Context<Self>,
    ) -> Option<(String, PageIndex, f32)> {
        if let Some(link) = self
            .session
            .read(cx)
            .links_for_page(page)
            .iter()
            .find(|l| l.rect.inflate(2.0).contains(norm_pt))
            .cloned()
        {
            if let LinkTarget::InternalPage {
                page: dest_page,
                top_norm_y,
            } = link.target
            {
                return Some((
                    format!("Link -> p.{}", dest_page.one_based()),
                    dest_page,
                    top_norm_y.unwrap_or(0.0),
                ));
            }
        }

        let layouts = self.session.update(cx, |s, _| s.ensure_all_layouts_now());
        let page_layout = layouts.iter().find(|l| l.page == page)?;
        let glyph_idx = page_layout.hit_test(norm_pt)?;
        let target = resolve_smart_jump_at(page, glyph_idx, &layouts)?;
        Some((target.label, target.page, (target.top_norm_y - 18.0).max(0.0)))
    }

    fn follow_internal_target(
        &mut self,
        page: PageIndex,
        top_norm_y: f32,
        cx: &mut Context<Self>,
    ) {
        if self.freeze_state.is_frozen() {
            cx.emit(ViewportEvent::RequestSmartLinkRoute {
                source_id: self.id,
                page,
                top_norm_y: Some(top_norm_y),
            });
        } else {
            self.jump_to_page_offset_recorded(page, top_norm_y, cx);
        }
    }

    fn handle_right_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        cx.emit(ViewportEvent::Focused(self.id));
        let Some((page, norm_pt, _)) = self.window_pos_to_page(event.position, cx) else {
            self.overview_preview = None;
            cx.notify();
            return;
        };
        if event.modifiers.alt {
            self.set_ruler_at_point(page, norm_pt, cx);
            return;
        }
        if let Some((label, dest_page, top_norm_y)) =
            self.resolve_target_at_point(page, norm_pt, cx)
        {
            self.overview_preview = Some(OverviewPreview {
                label,
                page: dest_page,
                top_norm_y,
                anchor_win_pt: event.position,
            });
            cx.notify();
        } else if self.overview_preview.is_some() {
            self.overview_preview = None;
            cx.notify();
        }
    }

    fn handle_middle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        cx.emit(ViewportEvent::Focused(self.id));
        let Some((page, norm_pt, _)) = self.window_pos_to_page(event.position, cx) else {
            return;
        };
        if let Some((_, dest_page, top_norm_y)) = self.resolve_target_at_point(page, norm_pt, cx) {
            self.overview_preview = None;
            self.follow_internal_target(dest_page, top_norm_y, cx);
        }
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left {
            return;
        }
        let drag = std::mem::replace(&mut self.drag_mode, DragMode::None);
        match drag {
            DragMode::AreaMarquee {
                page,
                start,
                current,
            } => {
                let rect = NormRect::from_points(start, current);
                if rect.width() > 8.0 && rect.height() > 8.0 {
                    let id = AnnotationId::new();
                    let ann = Annotation {
                        id,
                        pdf_object_id: None,
                        page,
                        kind: AnnotationKind::AreaNote,
                        quads: Vec::new(),
                        bounds: rect,
                        color: RgbaColor::ORANGE,
                        category: None,
                        selected_text: None,
                        note_contents: None,
                        modified_at: pdf_now_string(),
                    };
                    self.session.update(cx, |s, cx| s.add_annotation(ann, cx));
                    self.area_mode = false;
                    self.inspect_annotation(id, true, window, cx);
                }
                cx.notify();
            }
            DragMode::TextSelect { .. } | DragMode::None => {
                // If user just clicked (no drag selection), check links, annotations, then Smart Jump.
                let is_click = self
                    .selection
                    .is_none_or(TextSelectionRange::is_single_point);
                if is_click {
                    self.selection = None;
                    self.overview_preview = None;
                    if let Some((page, norm_pt, _)) = self.window_pos_to_page(event.position, cx) {
                        if let Some(link) = self
                            .session
                            .read(cx)
                            .links_for_page(page)
                            .iter()
                            .find(|l| l.rect.inflate(2.0).contains(norm_pt))
                            .cloned()
                        {
                            self.activate_link(link, cx);
                            return;
                        }

                        if let Some(hit_ann_id) = self
                            .session
                            .read(cx)
                            .annotations
                            .hit_test_page(page, norm_pt, 4.0)
                            .map(|a| a.id)
                        {
                            self.inspect_annotation(hit_ann_id, false, window, cx);
                            return;
                        }

                        // Heuristic Smart Jump fallback for unlinked references (Figure X, Theorem X, [N]).
                        if let Some((_, dest_page, top_norm_y)) =
                            self.resolve_target_at_point(page, norm_pt, cx)
                        {
                            self.follow_internal_target(dest_page, top_norm_y, cx);
                            return;
                        }
                    }
                    self.inspected_annotation = None;
                    self.editing_note = None;
                    cx.notify();
                }
            }
        }
    }

    fn activate_link(&mut self, link: LinkRegion, cx: &mut Context<Self>) {
        match link.target {
            LinkTarget::InternalPage { page, top_norm_y } => {
                self.follow_internal_target(page, top_norm_y.unwrap_or(0.0), cx);
            }
            LinkTarget::ExternalUrl(url) => {
                let opened = crate::external::open_external_url(&url);
                self.session.update(cx, |s, cx| {
                    s.status_message = Some((
                        if opened {
                            format!("Opened external link: {url}")
                        } else {
                            format!("External link: {url}")
                        },
                        !opened,
                    ));
                    cx.notify();
                });
            }
        }
    }

    fn handle_note_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((ann_id, ref mut buf)) = self.editing_note else {
            return;
        };
        let ks = &event.keystroke;
        if ks.key == "escape" {
            self.editing_note = None;
            window.focus(&self.focus_handle, cx);
            cx.notify();
            return;
        }
        if ks.key == "enter" && !ks.modifiers.shift {
            let final_text = buf.clone();
            self.editing_note = None;
            self.session.update(cx, |s, cx| {
                s.update_annotation_note(ann_id, Some(final_text), cx);
            });
            window.focus(&self.focus_handle, cx);
            cx.notify();
            return;
        }
        if ks.key == "backspace" {
            buf.pop();
            cx.notify();
            return;
        }
        if (ks.modifiers.platform || ks.modifiers.control) && ks.key == "v" {
            if let Some(clip) = cx.read_from_clipboard()
                && let Some(text) = clip.text()
            {
                buf.push_str(&text);
                cx.notify();
            }
            return;
        }
        if ks.key == "enter" && ks.modifiers.shift {
            buf.push('\n');
            cx.notify();
            return;
        }
        if !ks.modifiers.platform
            && !ks.modifiers.control
            && let Some(ch) = &ks.key_char
        {
            buf.push_str(ch);
            cx.notify();
        }
    }

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.hide_header {
            return div().into_any_element();
        }
        let is_frozen = self.freeze_state.is_frozen();
        let (trail_num, parent_num) = cx.global::<AppRegistry>().trail_info(self.id);
        let parent_id = cx.global::<AppRegistry>().parent_id(self.id);
        let is_flashed = cx.global::<AppRegistry>().flash_viewport == Some(self.id);
        let is_follower = self.portal_follower;
        let auto_scroll = self.auto_scroll_speed;

        let page_count = self.session.read(cx).page_count();
        let current_page = self.anchor.page.one_based();
        let zoom_label = match self.anchor.zoom {
            ZoomSpec::FitWidth => "Fit W".to_string(),
            ZoomSpec::FitContentWidth => "Fit C".to_string(),
            ZoomSpec::FitPage => "Fit P".to_string(),
            ZoomSpec::Fixed(s) => format!("{:.0}%", s * 100.0),
        };

        let header_bg = if self.frozen_feedback_active {
            rgb(0xfef3c7)
        } else if is_frozen {
            rgb(0xfffbeb)
        } else if is_follower {
            rgb(0xf3e8ff)
        } else if is_flashed {
            rgb(0xe0f2fe)
        } else {
            rgb(0xf4f4f5)
        };

        let border_col = if is_frozen {
            rgb(0xf59e0b)
        } else if is_follower {
            rgb(0x9333ea)
        } else {
            rgb(0xe4e4e7)
        };

        let mut header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            .px_2()
            .py_1()
            .bg(header_bg)
            .border_b_1()
            .border_color(border_col)
            .text_xs()
            .text_color(rgb(0x18181b));

        // Left group: Lineage badge + Freeze button + Page nav
        let mut left = div().flex().items_center().gap_1p5();
        left = left.child(
            div()
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .bg(if is_frozen {
                    rgb(0xfde68a)
                } else {
                    rgb(0xdbeafe)
                })
                .text_color(if is_frozen {
                    rgb(0x92400e)
                } else {
                    rgb(0x1e40af)
                })
                .child(format!("#{trail_num}")),
        );

        if let (Some(pnum), Some(pid)) = (parent_num, parent_id) {
            left = left.child(
                div()
                    .id(("parent-link", self.id.0))
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0xe4e4e7)))
                    .text_color(rgb(0x52525b))
                    .child(format!("<- #{pnum}"))
                    .on_click(move |_, window, cx| {
                        focus_viewport_globally(pid, Some(window), cx);
                    }),
            );
        }

        left = left
            .child(
                div()
                    .id(("freeze-btn", self.id.0))
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .bg(if is_frozen {
                        rgb(0xd97706)
                    } else {
                        rgb(0xe4e4e7)
                    })
                    .text_color(if is_frozen {
                        rgb(0xffffff)
                    } else {
                        rgb(0x27272a)
                    })
                    .hover(|s| s.opacity(0.9))
                    .child(if is_frozen { "LOCKED [F]" } else { "Freeze [F]" })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_freeze(cx))),
            )
            .child(
                div()
                    .id(("portal-btn", self.id.0))
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .font_weight(FontWeight::MEDIUM)
                    .bg(if is_follower {
                        rgb(0x7e22ce)
                    } else {
                        rgb(0xe4e4e7)
                    })
                    .text_color(if is_follower {
                        rgb(0xffffff)
                    } else {
                        rgb(0x27272a)
                    })
                    .hover(|s| s.opacity(0.9))
                    .child(match &self.active_portal_label {
                        Some(lbl) if is_follower => format!("Portal: {lbl}"),
                        _ if is_follower => "Portal [⇧P]".to_string(),
                        _ => "Portal".to_string(),
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.emit(ViewportEvent::RequestTogglePortalFollower(this.id));
                    })),
            )
            .when_some(auto_scroll, |row, speed| {
                row.child(
                    div()
                        .id(("autoscroll-pill", self.id.0))
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x047857))
                        .text_color(rgb(0xffffff))
                        .cursor_pointer()
                        .child(format!("Auto {:.0}px/s", speed))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_auto_scroll(cx))),
                )
            })
            .child(
                div()
                    .id(("prev-pg", self.id.0))
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(0xe4e4e7))
                    .hover(|s| s.bg(rgb(0xd4d4d8)))
                    .child("<")
                    .on_click(cx.listener(|this, _, _, cx| this.prev_page(cx))),
            )
            .child(
                div()
                    .id(("goto-pg-pill", self.id.0))
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0xe4e4e7)))
                    .text_color(if self.page_count_prefix.is_empty() {
                        rgb(0x3f3f46)
                    } else {
                        rgb(0x1d4ed8)
                    })
                    .child(if self.page_count_prefix.is_empty() {
                        format!("p.{current_page}/{page_count}")
                    } else {
                        format!("Go: {}_ ({current_page}/{page_count})", self.page_count_prefix)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.emit(ViewportEvent::RequestGoToPage(this.id));
                    })),
            )
            .child(
                div()
                    .id(("next-pg", self.id.0))
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(rgb(0xe4e4e7))
                    .hover(|s| s.bg(rgb(0xd4d4d8)))
                    .child(">")
                    .on_click(cx.listener(|this, _, _, cx| this.next_page(cx))),
            );

        // Center/Right group: Zoom + Area mode + Freeze-and-Branch actions
        let right = div()
            .flex()
            .items_center()
            .gap_1()
            .child(pane_btn(
                ("zoom-out", self.id.0),
                "-",
                cx.listener(|this, _, _, cx| this.zoom_out(cx)),
            ))
            .child(pane_btn(
                ("zoom-fit", self.id.0),
                zoom_label,
                cx.listener(|this, _, _, cx| this.zoom_fit_width(cx)),
            ))
            .child(pane_btn(
                ("zoom-fitc", self.id.0),
                "FitC",
                cx.listener(|this, _, _, cx| this.zoom_fit_content_width(cx)),
            ))
            .child(pane_btn(
                ("zoom-in", self.id.0),
                "+",
                cx.listener(|this, _, _, cx| this.zoom_in(cx)),
            ))
            .child(
                div()
                    .id(("area-mode", self.id.0))
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .bg(if self.area_mode {
                        rgb(0xf97316)
                    } else {
                        rgb(0xe4e4e7)
                    })
                    .text_color(if self.area_mode {
                        rgb(0xffffff)
                    } else {
                        rgb(0x27272a)
                    })
                    .child("Box")
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_area_mode(cx))),
            )
            .child(pane_btn(
                ("split-r", self.id.0),
                "Freeze+Split R",
                cx.listener(|this, _, _, cx| {
                    cx.emit(ViewportEvent::RequestFreezeAndSplit {
                        id: this.id,
                        axis: Axis::Horizontal,
                    });
                }),
            ))
            .child(pane_btn(
                ("split-d", self.id.0),
                "Split D",
                cx.listener(|this, _, _, cx| {
                    cx.emit(ViewportEvent::RequestFreezeAndSplit {
                        id: this.id,
                        axis: Axis::Vertical,
                    });
                }),
            ))
            .child(pane_btn(
                ("new-win", self.id.0),
                "+Win",
                cx.listener(|this, _, _, cx| {
                    cx.emit(ViewportEvent::RequestFreezeAndNewWindow(this.id));
                }),
            ))
            .child(pane_btn(
                ("pop-out", self.id.0),
                "Pop",
                cx.listener(|this, _, _, cx| {
                    cx.emit(ViewportEvent::RequestDetachToWindow(this.id));
                }),
            ))
            .child(pane_btn(
                ("close-pane", self.id.0),
                "x",
                cx.listener(|this, _, _, cx| {
                    cx.emit(ViewportEvent::RequestClose(this.id));
                }),
            ));

        header = header.child(left).child(right);
        header.into_any_element()
    }

    fn render_selection_popover(
        &self,
        placements: &[PagePlacement],
        scale: f32,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let sel = self.selection?;
        if sel.is_single_point() {
            return None;
        }
        let quads = self.selection_quads_by_page(cx);
        let (first_page, first_quads) = quads.first()?;
        let first_quad = first_quads.first()?;
        let placement = placements.iter().find(|p| p.page == *first_page)?;
        let rect = first_quad.bounding_rect();

        let vw = bounds.size.width.as_f32().max(260.0);
        let x = (placement.local_x + rect.min_x * scale).clamp(8.0, (vw - 290.0).max(8.0));
        let y = (placement.local_y + rect.min_y * scale - 38.0).max(8.0);

        let swatches = RgbaColor::PRESETS
            .into_iter()
            .take(4)
            .enumerate()
            .map(|(idx, (_name, color))| {
                let swatch_u32 = color.to_u32_rgba(Some(1.0));
                div()
                    .id(("sel-swatch", idx))
                    .size(px(16.))
                    .rounded_full()
                    .bg(rgba(swatch_u32))
                    .border_1()
                    .border_color(rgb(0x52525b))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.commit_text_annotation(
                            AnnotationKind::Highlight,
                            None,
                            color,
                            false,
                            window,
                            cx,
                        );
                    }))
            });

        Some(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .flex()
                .items_center()
                .gap_1p5()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(rgb(0x18181b))
                .text_color(rgb(0xffffff))
                .text_xs()
                .border_1()
                .border_color(rgb(0x3f3f46))
                .shadow_md()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id(("sel-copy", self.id.0))
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x3f3f46)))
                        .child("Copy")
                        .on_click(cx.listener(|this, _, _, cx| this.copy_selection(cx))),
                )
                .children(swatches)
                .child(
                    div()
                        .id(("sel-ul", self.id.0))
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x3f3f46)))
                        .child("U")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.underline_selection(window, cx);
                        })),
                )
                .child(
                    div()
                        .id(("sel-strike", self.id.0))
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x3f3f46)))
                        .child("S")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.strikeout_selection(window, cx);
                        })),
                )
                .child(
                    div()
                        .id(("sel-note", self.id.0))
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x2563eb))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9))
                        .child("+Note")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.annotate_selection_with_note(window, cx);
                        })),
                )
                .child(
                    div()
                        .id(("sel-scholar", self.id.0))
                        .px_1p5()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0x047857))
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9))
                        .child("Scholar")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.lookup_selection_scholar(cx);
                        })),
                )
                .into_any_element(),
        )
    }

    fn render_annotation_popover(
        &self,
        placements: &[PagePlacement],
        scale: f32,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let ann_id = self.inspected_annotation?;
        let ann = self.session.read(cx).annotations.get(ann_id)?.clone();
        let placement = placements.iter().find(|p| p.page == ann.page)?;

        let vw = bounds.size.width.as_f32().max(280.0);
        let vh = bounds.size.height.as_f32().max(220.0);
        let x = (placement.local_x + ann.bounds.min_x * scale).clamp(8.0, (vw - 290.0).max(8.0));
        let y = (placement.local_y + ann.bounds.max_y * scale + 6.0).clamp(8.0, (vh - 150.0).max(8.0));

        let is_editing = self
            .editing_note
            .as_ref()
            .is_some_and(|(id, _)| *id == ann_id);
        let note_draft = self
            .editing_note
            .as_ref()
            .filter(|(id, _)| *id == ann_id)
            .map(|(_, s)| s.clone())
            .unwrap_or_else(|| ann.note_contents.clone().unwrap_or_default());

        let swatches = RgbaColor::PRESETS
            .into_iter()
            .enumerate()
            .map(|(idx, (_name, color))| {
                let swatch_u32 = color.to_u32_rgba(Some(1.0));
                div()
                    .id(("ann-swatch", idx))
                    .size(px(16.))
                    .rounded_full()
                    .bg(rgba(swatch_u32))
                    .border_1()
                    .border_color(rgb(0x52525b))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.session
                            .update(cx, |s, cx| s.update_annotation_color(ann_id, color, cx));
                    }))
            });

        let note_area: AnyElement = if is_editing {
            div()
                .id(("note-editor", self.id.0))
                .key_context(NOTE_EDITOR_CONTEXT)
                .track_focus(&self.note_focus_handle)
                .on_key_down(cx.listener(Self::handle_note_key_down))
                .w_full()
                .min_h(px(54.))
                .p_2()
                .rounded_sm()
                .bg(rgb(0xffffff))
                .text_color(rgb(0x18181b))
                .border_1()
                .border_color(rgb(0x2563eb))
                .text_xs()
                .child(if note_draft.is_empty() {
                    "Type note (Enter to save, Shift+Enter newline, Esc cancel)...".to_string()
                } else {
                    format!("{note_draft}|")
                })
                .into_any_element()
        } else {
            div()
                .id(("note-preview", self.id.0))
                .w_full()
                .p_1p5()
                .rounded_sm()
                .bg(rgb(0x27272a))
                .text_xs()
                .text_color(if note_draft.is_empty() {
                    rgb(0xa1a1aa)
                } else {
                    rgb(0xf4f4f5)
                })
                .cursor_pointer()
                .child(if note_draft.is_empty() {
                    "Click to add a note...".to_string()
                } else {
                    note_draft
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.inspect_annotation(ann_id, true, window, cx);
                }))
                .into_any_element()
        };

        Some(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(280.))
                .flex()
                .flex_col()
                .gap_2()
                .p_2p5()
                .rounded_md()
                .bg(rgb(0x18181b))
                .text_color(rgb(0xffffff))
                .text_xs()
                .border_1()
                .border_color(rgb(0x3f3f46))
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .children(swatches)
                            .when_some(ann.category, |row, cat| {
                                let (lbl, _) = highlight_category_spec(cat);
                                row.child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x27272a))
                                        .text_color(rgb(0xd4d4d8))
                                        .child(format!("[{cat}] {lbl}")),
                                )
                            }),
                    )
                    .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id(("ann-del", self.id.0))
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x991b1b))
                                        .cursor_pointer()
                                        .child("Delete")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.session.update(cx, |s, cx| {
                                                s.delete_annotation(ann_id, cx)
                                            });
                                            this.inspected_annotation = None;
                                            this.editing_note = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .id(("ann-close", self.id.0))
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x3f3f46))
                                        .cursor_pointer()
                                        .child("x")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.inspected_annotation = None;
                                            this.editing_note = None;
                                            window.focus(&this.focus_handle, cx);
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .child(note_area)
                .when(is_editing, |col| {
                    col.child(
                        div()
                            .flex()
                            .justify_end()
                            .child(
                                div()
                                    .id(("note-save", self.id.0))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x2563eb))
                                    .cursor_pointer()
                                    .child("Save Note")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some((id, txt)) = this.editing_note.take() {
                                            this.session.update(cx, |s, cx| {
                                                s.update_annotation_note(id, Some(txt), cx);
                                            });
                                        }
                                        window.focus(&this.focus_handle, cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                })
                .into_any_element(),
        )
    }

    fn render_overview_popover(
        &self,
        bounds: Bounds<Pixels>,
        overview_img: Option<Arc<gpui::RenderImage>>,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let preview = self.overview_preview.as_ref()?.clone();
        let meta = self
            .session
            .read(cx)
            .page_metas
            .get(preview.page.as_usize())
            .cloned()?;
        let card_w = 436.0_f32;
        let preview_h = 218.0_f32;
        let vw = bounds.size.width.as_f32().max(456.0);
        let vh = bounds.size.height.as_f32().max(280.0);
        let local_x = (preview.anchor_win_pt.x.as_f32() - bounds.origin.x.as_f32() + 12.0)
            .clamp(8.0, (vw - card_w - 8.0).max(8.0));
        let local_y = (preview.anchor_win_pt.y.as_f32() - bounds.origin.y.as_f32() + 12.0)
            .clamp(8.0, (vh - (preview_h + 44.0) - 8.0).max(8.0));

        let scale = ((card_w - 8.0) / meta.norm_width.max(1.0)).clamp(0.35, 2.5);
        let img_w = meta.norm_width * scale;
        let img_h = meta.norm_height * scale;
        let offset_y = ((preview.top_norm_y - 16.0).max(0.0) * scale)
            .clamp(0.0, (img_h - preview_h).max(0.0));
        let session_ref = self.session.read(cx);
        let color_mode = session_ref.color_mode;
        let custom_bg = session_ref.custom_bg_rgb;
        let page_bg = match color_mode {
            ColorMode::Light => rgb(0xffffff),
            ColorMode::Sepia => rgb(0xf7efe0),
            ColorMode::Dark => rgb(0x14161c),
            ColorMode::Custom => {
                rgb(((custom_bg[0] as u32) << 16) | ((custom_bg[1] as u32) << 8) | (custom_bg[2] as u32))
            }
        };

        Some(
            div()
                .id(("overview-popover", self.id.0))
                .absolute()
                .left(px(local_x))
                .top(px(local_y))
                .w(px(card_w))
                .flex()
                .flex_col()
                .rounded_md()
                .bg(rgb(0x18181b))
                .border_1()
                .border_color(rgb(0x3b82f6))
                .shadow_lg()
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_2p5()
                        .py_1p5()
                        .border_b_1()
                        .border_color(rgb(0x27272a))
                        .text_xs()
                        .text_color(rgb(0xf4f4f5))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!(
                                    "Overview: {} (p.{})",
                                    preview.label,
                                    preview.page.one_based()
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .id(("ov-jump", self.id.0))
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x2563eb))
                                        .cursor_pointer()
                                        .child("Jump [Enter]")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.overview_jump(cx);
                                        })),
                                )
                                .child(
                                    div()
                                        .id(("ov-split", self.id.0))
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x27272a))
                                        .hover(|s| s.bg(rgb(0x3f3f46)))
                                        .cursor_pointer()
                                        .child("Split [S]")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.overview_split(cx);
                                        })),
                                )
                                .child(
                                    div()
                                        .id(("ov-close", self.id.0))
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x3f3f46))
                                        .hover(|s| s.bg(rgb(0x52525b)))
                                        .cursor_pointer()
                                        .child("x [Esc]")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.close_overview(cx);
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(preview_h))
                        .p_1()
                        .bg(page_bg)
                        .child(
                            canvas(
                                |_, _, _| {},
                                move |cb, _, window, _| {
                                    window.with_content_mask(
                                        Some(ContentMask {
                                            bounds: cb,
                                            fade_out: Default::default(),
                                        }),
                                        |window| {
                                            window.paint_quad(
                                                PaintQuad::new(cb).background(page_bg),
                                            );
                                            if let Some(ref img) = overview_img {
                                                let img_bounds = Bounds::new(
                                                    point(
                                                        cb.origin.x
                                                            + px(((cb.size.width.as_f32()
                                                                - img_w)
                                                                * 0.5)
                                                                .max(0.0)),
                                                        cb.origin.y - px(offset_y),
                                                    ),
                                                    size(px(img_w), px(img_h)),
                                                );
                                                let _ = window.paint_image(
                                                    cb,
                                                    img_bounds,
                                                    Default::default(),
                                                    0.0,
                                                    img.clone(),
                                                    0,
                                                    false,
                                                );
                                            }
                                        },
                                    );
                                },
                            )
                            .size_full(),
                        ),
                )
                .into_any_element(),
        )
    }
}

impl Render for PdfViewport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if matches!(self.anchor.zoom, ZoomSpec::FitContentWidth) {
            let anchor_page = self.anchor.page;
            self.session.update(cx, |s, _| {
                s.content_x_bounds(anchor_page);
            });
        }
        let is_frozen = self.freeze_state.is_frozen();
        let is_focused = self.focus_handle.is_focused(window);
        let bounds = self.last_canvas_bounds.get();
        let vw = bounds.size.width.as_f32().max(320.0);
        let vh = bounds.size.height.as_f32().max(240.0);
        let (scale, _, _, placements) = self.compute_layout(vw, vh, cx);

        // Request renders for visible & adjacent pages (plus Overview preview page if open).
        let hidpi = window.scale_factor();
        let target_bucket = scale_to_zoom_bucket(scale * hidpi);
        let overview_bucket = scale_to_zoom_bucket(1.2 * hidpi);
        let overview_page = self.overview_preview.as_ref().map(|p| p.page);
        let visible_pages: Vec<PagePlacement> = placements
            .iter()
            .filter(|p| p.local_y + p.pixel_h >= -240.0 && p.local_y <= vh + 240.0)
            .cloned()
            .collect();

        let mut page_images = Vec::with_capacity(visible_pages.len());
        let mut overview_img: Option<Arc<gpui::RenderImage>> = None;
        self.session.update(cx, |session, cx| {
            for p in &visible_pages {
                let (img_opt, exact) = session.best_available_bitmap(p.page, target_bucket);
                if !exact {
                    session.request_page_bitmap(p.page, target_bucket, cx);
                }
                page_images.push((p.page, img_opt));
            }
            if let Some(ov_page) = overview_page {
                let (img_opt, exact) = session.best_available_bitmap(ov_page, overview_bucket);
                if !exact {
                    session.request_page_bitmap(ov_page, overview_bucket, cx);
                }
                overview_img = img_opt;
            }
        });

        let ruler_line = self.ruler_line;
        let ruler_rect = if let Some((rp, ridx)) = ruler_line {
            let layout = self.session.update(cx, |s, _| s.ensure_text_layout(rp));
            layout.lines.get(ridx).map(|l| (rp, l.bounds))
        } else {
            None
        };

        let selection_quads = self.selection_quads_by_page(cx);
        let marquee = match &self.drag_mode {
            DragMode::AreaMarquee {
                page,
                start,
                current,
            } => Some((*page, NormRect::from_points(*start, *current))),
            _ => None,
        };

        // Snapshot page annotations, links, color mode, portals, SyncTeX, and active search quads for the paint closure.
        let session_ref = self.session.read(cx);
        let color_mode = session_ref.color_mode;
        let custom_bg = session_ref.custom_bg_rgb;
        let synctex_highlight = session_ref.synctex_highlight;
        let page_portals: Vec<Vec<(u64, f32)>> = visible_pages
            .iter()
            .map(|p| {
                session_ref
                    .portals
                    .iter()
                    .filter(|portal| portal.src_page == p.page)
                    .map(|portal| (portal.id, portal.src_focal_y))
                    .collect()
            })
            .collect();
        let mut page_annots = Vec::with_capacity(visible_pages.len());
        let mut page_links = Vec::with_capacity(visible_pages.len());
        let mut page_search_quads: Vec<Vec<(NormQuad, bool)>> =
            Vec::with_capacity(visible_pages.len());
        let active_search_idx = session_ref.search.active_idx;
        for p in &visible_pages {
            let anns: Vec<Annotation> = session_ref
                .annotations
                .for_page(p.page)
                .into_iter()
                .cloned()
                .collect();
            let links = session_ref.links_for_page(p.page).to_vec();
            let mut s_quads = Vec::new();
            for (midx, m) in session_ref.search.matches.iter().enumerate() {
                if m.page == p.page {
                    let is_active = midx == active_search_idx;
                    for q in &m.quads {
                        s_quads.push((*q, is_active));
                    }
                }
            }
            page_annots.push((p.page, anns));
            page_links.push((p.page, links));
            page_search_quads.push(s_quads);
        }

        let hovered_link = self.hovered_link;
        let bounds_cell = self.last_canvas_bounds.clone();
        let placements_for_paint = visible_pages.clone();
        let (canvas_bg, paper_bg, paper_border) = match color_mode {
            ColorMode::Light => (rgb(0xe4e4e7), rgb(0xffffff), rgb(0xcbd5e1)),
            ColorMode::Sepia => (rgb(0xd6cfc2), rgb(0xf7efe0), rgb(0xb8ae9c)),
            ColorMode::Dark => (rgb(0x09090b), rgb(0x14161c), rgb(0x27272a)),
            ColorMode::Custom => {
                let p_u32 = ((custom_bg[0] as u32) << 16)
                    | ((custom_bg[1] as u32) << 8)
                    | (custom_bg[2] as u32);
                let c_bg = rgb(((custom_bg[0].saturating_sub(14) as u32) << 16)
                    | ((custom_bg[1].saturating_sub(14) as u32) << 8)
                    | (custom_bg[2].saturating_sub(14) as u32));
                (c_bg, rgb(p_u32), rgb(0x3f3f46))
            }
        };
        let canvas_el = canvas(
            move |canvas_bounds, _, _| {
                bounds_cell.set(canvas_bounds);
            },
            move |canvas_bounds, _, window, _| {
                window.with_content_mask(
                    Some(ContentMask {
                        bounds: canvas_bounds,
                        fade_out: Default::default(),
                    }),
                    |window| {
                        // Viewport background
                        window.paint_quad(
                            PaintQuad::new(canvas_bounds).background(canvas_bg),
                        );

                        for (idx, p) in placements_for_paint.iter().enumerate() {
                            let page_bounds = Bounds::new(
                                point(
                                    canvas_bounds.origin.x + px(p.local_x),
                                    canvas_bounds.origin.y + px(p.local_y),
                                ),
                                size(px(p.pixel_w), px(p.pixel_h)),
                            );

                            // Paper card
                            window.paint_quad(
                                PaintQuad::new(page_bounds)
                                    .background(paper_bg)
                                    .border_widths(px(1.))
                                    .border_color(paper_border),
                            );

                            if let Some((_, Some(img))) = page_images.get(idx) {
                                let _ = window.paint_image(
                                    page_bounds,
                                    page_bounds,
                                    Default::default(),
                                    0.0,
                                    img.clone(),
                                    0,
                                    false,
                                );
                            }

                            // Paint Visual Reading Ruler line band
                            if let Some((rp, rbox)) = ruler_rect
                                && rp == p.page
                            {
                                let rb = norm_rect_to_bounds(rbox.inflate(2.0), page_bounds, scale);
                                let (fill, bar) = match color_mode {
                                    ColorMode::Dark | ColorMode::Custom => {
                                        (rgba(0x38bdf830), rgb(0x38bdf8))
                                    }
                                    _ => (rgba(0xfbbf242e), rgb(0xd97706)),
                                };
                                window.paint_quad(
                                    PaintQuad::new(rb).background(fill).corner_radii(px(2.)),
                                );
                                let accent = Bounds::new(
                                    point(rb.origin.x - px(5.), rb.origin.y),
                                    size(px(3.), rb.size.height),
                                );
                                window.paint_quad(
                                    PaintQuad::new(accent).background(bar).corner_radii(px(1.5)),
                                );
                            }

                            // Paint Scroll-Linked Portal margin anchors
                            if let Some(portals_on_page) = page_portals.get(idx) {
                                for &(_pid, fy) in portals_on_page {
                                    let py = page_bounds.origin.y
                                        + px((fy * p.norm_height * scale).clamp(4.0, p.pixel_h - 14.0));
                                    let marker = Bounds::new(
                                        point(page_bounds.origin.x - px(7.), py),
                                        size(px(5.), px(14.)),
                                    );
                                    window.paint_quad(
                                        PaintQuad::new(marker)
                                            .background(rgb(0x9333ea))
                                            .corner_radii(px(2.5)),
                                    );
                                }
                            }

                            // Paint SyncTeX forward-search highlight
                            if let Some((sp, srect)) = synctex_highlight
                                && sp == p.page
                            {
                                let sb = norm_rect_to_bounds(srect.inflate(3.0), page_bounds, scale);
                                window.paint_quad(
                                    PaintQuad::new(sb)
                                        .background(rgba(0x06b6d444))
                                        .border_widths(px(1.5))
                                        .border_color(rgb(0x0891b2))
                                        .corner_radii(px(3.)),
                                );
                            }

                            // Paint saved/live annotations on this page
                            if let Some((_, anns)) = page_annots.get(idx) {
                                for ann in anns {
                                    paint_annotation_overlay(ann, page_bounds, scale, window);
                                }
                            }

                            // Paint full-text search matches
                            if let Some(s_quads) = page_search_quads.get(idx) {
                                for &(q, is_active) in s_quads {
                                    let sb = norm_rect_to_bounds(
                                        q.bounding_rect(),
                                        page_bounds,
                                        scale,
                                    );
                                    let bg = if is_active {
                                        rgba(0xf9731670)
                                    } else {
                                        rgba(0xfef08a66)
                                    };
                                    let mut pq = PaintQuad::new(sb)
                                        .background(bg)
                                        .corner_radii(px(2.));
                                    if is_active {
                                        pq = pq
                                            .border_widths(px(1.5))
                                            .border_color(rgb(0xea580c));
                                    }
                                    window.paint_quad(pq);
                                }
                            }

                            // Paint link hover outline
                            if let Some((_, links)) = page_links.get(idx) {
                                for (lidx, link) in links.iter().enumerate() {
                                    if hovered_link == Some((p.page, lidx)) {
                                        let lb = norm_rect_to_bounds(
                                            link.rect,
                                            page_bounds,
                                            scale,
                                        );
                                        window.paint_quad(
                                            PaintQuad::new(lb)
                                                .background(rgba(0x2563eb1a))
                                                .border_widths(px(1.5))
                                                .border_color(rgb(0x2563eb))
                                                .corner_radii(px(2.)),
                                        );
                                    }
                                }
                            }

                            // Paint active text selection quads
                            for (sp, quads) in &selection_quads {
                                if *sp == p.page {
                                    for q in quads {
                                        let qb = norm_rect_to_bounds(
                                            q.bounding_rect(),
                                            page_bounds,
                                            scale,
                                        );
                                        window.paint_quad(
                                            PaintQuad::new(qb)
                                                .background(rgba(0x2563eb4d))
                                                .corner_radii(px(1.5)),
                                        );
                                    }
                                }
                            }

                            // Paint Alt+Drag area marquee
                            if let Some((mp, mr)) = marquee
                                && mp == p.page
                            {
                                let mb = norm_rect_to_bounds(mr, page_bounds, scale);
                                window.paint_quad(
                                    PaintQuad::new(mb)
                                        .background(rgba(0xf9731628))
                                        .border_widths(px(2.))
                                        .border_color(rgb(0xea580c))
                                        .corner_radii(px(2.)),
                                );
                            }
                        }
                    },
                );
            },
        )
        .size_full();

        let sel_popover = self.render_selection_popover(&placements, scale, bounds, cx);
        let ann_popover = self.render_annotation_popover(&placements, scale, bounds, cx);
        let overview_popover = self.render_overview_popover(bounds, overview_img, cx);

        let outer_border = if self.hide_header {
            rgb(0x09090b)
        } else if is_frozen {
            rgb(0xd97706)
        } else if self.portal_follower {
            rgb(0x9333ea)
        } else if is_focused {
            rgb(0x2563eb)
        } else {
            rgb(0xd4d4d8)
        };

        let cursor = if self.hovered_link.is_some() {
            CursorStyle::PointingHand
        } else if self.area_mode {
            CursorStyle::Crosshair
        } else {
            CursorStyle::IBeam
        };

        let mark_prompt = self.pending_mark_op.map(|op| {
            let msg = match op {
                PendingMarkOp::Set => "Set mark: press [a-z0-9] (Esc cancels)",
                PendingMarkOp::Jump => "Jump to mark: press [a-z0-9] (Esc cancels)",
                PendingMarkOp::TypedHighlight => {
                    "Category highlight: press [a-z] bucket (t=Theorem, d=Def, r=Result, ... Esc cancels)"
                }
            };
            div()
                .absolute()
                .top(px(10.))
                .left(px(10.))
                .px_2p5()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x1e3a8a))
                .text_color(rgb(0xe0f2fe))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .shadow_md()
                .child(msg)
        });

        let hint_banner = self.hint_mode.as_ref().map(|hm| {
            let mode_title = match hm.kind {
                HintModeKind::Jump => "Hint Jump",
                HintModeKind::Overview => "Hint Overview",
                HintModeKind::SelectStart => "Hint Select Start",
                HintModeKind::SelectEnd { .. } => "Hint Select End",
            };
            let filter_str = if hm.typed.is_empty() {
                "type 2-letter label".to_string()
            } else {
                format!("typed: \"{}\"", hm.typed.to_ascii_uppercase())
            };
            div()
                .absolute()
                .top(px(10.))
                .left(px(10.))
                .px_2p5()
                .py_1()
                .rounded_sm()
                .bg(rgb(0x92400e))
                .text_color(rgb(0xfef3c7))
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .shadow_md()
                .child(format!("{mode_title}: {filter_str} (Esc cancel)"))
        });

        let mut hint_pills: Vec<AnyElement> = Vec::new();
        if let Some(ref hm) = self.hint_mode {
            for (idx, cand) in hm.candidates.iter().enumerate() {
                if !cand.label.starts_with(&hm.typed) {
                    continue;
                }
                let Some(p) = visible_pages.iter().find(|p| p.page == cand.page) else {
                    continue;
                };
                let hx = (p.local_x + cand.rect.min_x * scale - 2.0)
                    .clamp(4.0, (vw - 36.0).max(4.0));
                let hy = (p.local_y + cand.rect.min_y * scale - 15.0)
                    .clamp(4.0, (vh - 20.0).max(4.0));
                let display_lbl = cand.label.to_ascii_uppercase();
                hint_pills.push(
                    div()
                        .id(("hint-pill", (self.id.0 << 12) | idx as u64))
                        .absolute()
                        .left(px(hx))
                        .top(px(hy))
                        .px_1()
                        .py_0p5()
                        .rounded_sm()
                        .bg(rgb(0xf59e0b))
                        .border_1()
                        .border_color(rgb(0x451a03))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0x09090b))
                        .shadow_sm()
                        .child(display_lbl)
                        .into_any_element(),
                );
            }
        }

        let mut vp_context = VIEWPORT_CONTEXT.to_string();
        if self.pending_mark_op.is_some() {
            vp_context.push(' ');
            vp_context.push_str(MARK_PENDING_CONTEXT);
        }
        if self.hint_mode.is_some() {
            vp_context.push(' ');
            vp_context.push_str(HINT_MODE_CONTEXT);
        }

        div()
            .id(("viewport-root", self.id.0))
            .key_context(vp_context.as_str())
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_viewport_key_down))
            .size_full()
            .flex()
            .flex_col()
            .border_2()
            .border_color(outer_border)
            .bg(rgb(0xe4e4e7))
            .on_action(cx.listener(|this, _: &ToggleFreeze, _, cx| this.toggle_freeze(cx)))
            .on_action(cx.listener(|this, _: &FreezeAndSplitRight, _, cx| {
                cx.emit(ViewportEvent::RequestFreezeAndSplit {
                    id: this.id,
                    axis: Axis::Horizontal,
                });
            }))
            .on_action(cx.listener(|this, _: &FreezeAndSplitDown, _, cx| {
                cx.emit(ViewportEvent::RequestFreezeAndSplit {
                    id: this.id,
                    axis: Axis::Vertical,
                });
            }))
            .on_action(cx.listener(|this, _: &FreezeAndNewWindow, _, cx| {
                cx.emit(ViewportEvent::RequestFreezeAndNewWindow(this.id));
            }))
            .on_action(cx.listener(|this, _: &DetachPaneToWindow, _, cx| {
                cx.emit(ViewportEvent::RequestDetachToWindow(this.id));
            }))
            .on_action(cx.listener(|this, _: &ClosePane, _, cx| {
                cx.emit(ViewportEvent::RequestClose(this.id));
            }))
            .on_action(cx.listener(|this, _: &CopySelection, _, cx| this.copy_selection(cx)))
            .on_action(cx.listener(|this, _: &LookupScholar, _, cx| {
                this.lookup_selection_scholar(cx);
            }))
            .on_action(cx.listener(|this, _: &HighlightSelection, window, cx| {
                this.highlight_selection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &AnnotateWithNote, window, cx| {
                this.annotate_selection_with_note(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom_in(cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom_out(cx)))
            .on_action(cx.listener(|this, _: &ZoomFitWidth, _, cx| this.zoom_fit_width(cx)))
            .on_action(cx.listener(|this, _: &ZoomFitContentWidth, _, cx| {
                this.zoom_fit_content_width(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleReadingRuler, _, cx| {
                this.toggle_reading_ruler(cx);
            }))
            .on_action(cx.listener(|this, _: &NextPage, _, cx| this.next_page(cx)))
            .on_action(cx.listener(|this, _: &PrevPage, _, cx| this.prev_page(cx)))
            .on_action(cx.listener(|this, _: &JumpBack, _, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &JumpForward, _, cx| this.go_forward(cx)))
            .on_action(cx.listener(|this, _: &ScrollLineDown, _, cx| this.scroll_line_down(cx)))
            .on_action(cx.listener(|this, _: &ScrollLineUp, _, cx| this.scroll_line_up(cx)))
            .on_action(cx.listener(|this, _: &ScrollHalfPageDown, _, cx| this.scroll_half_page_down(cx)))
            .on_action(cx.listener(|this, _: &ScrollHalfPageUp, _, cx| this.scroll_half_page_up(cx)))
            .on_action(cx.listener(|this, _: &ScrollScreenDown, _, cx| this.scroll_screen_down(cx)))
            .on_action(cx.listener(|this, _: &ScrollScreenUp, _, cx| this.scroll_screen_up(cx)))
            .on_action(cx.listener(|this, _: &FirstPage, _, cx| this.go_first_page(cx)))
            .on_action(cx.listener(|this, _: &LastPage, _, cx| this.go_last_page(cx)))
            .on_action(cx.listener(|this, _: &BeginSetMark, _, cx| {
                this.begin_mark_op(PendingMarkOp::Set, cx);
            }))
            .on_action(cx.listener(|this, _: &BeginJumpMark, _, cx| {
                this.begin_mark_op(PendingMarkOp::Jump, cx);
            }))
            .on_action(cx.listener(|this, _: &BeginTypedHighlight, _, cx| {
                this.begin_typed_highlight(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleAreaMode, _, cx| this.toggle_area_mode(cx)))
            .on_action(cx.listener(|this, _: &LinkPortal, _, cx| this.link_portal(cx)))
            .on_action(cx.listener(|this, _: &TogglePortalFollower, _, cx| {
                cx.emit(ViewportEvent::RequestTogglePortalFollower(this.id));
            }))
            .on_action(cx.listener(|this, _: &DeleteNearestPortal, _, cx| {
                this.delete_nearest_portal(cx);
            }))
            .on_action(cx.listener(|this, _: &BeginHintJump, _, cx| {
                this.begin_hint_mode(HintModeKind::Jump, cx);
            }))
            .on_action(cx.listener(|this, _: &BeginHintOverview, _, cx| {
                this.begin_hint_mode(HintModeKind::Overview, cx);
            }))
            .on_action(cx.listener(|this, _: &BeginHintSelect, _, cx| {
                this.begin_hint_mode(HintModeKind::SelectStart, cx);
            }))
            .on_action(cx.listener(|this, _: &SynctexInverseJump, _, cx| {
                this.synctex_inverse_here(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleAutoScroll, _, cx| {
                this.toggle_auto_scroll(cx);
            }))
            .on_action(cx.listener(|this, _: &AutoScrollFaster, _, cx| {
                this.adjust_auto_scroll_speed(20.0, cx);
            }))
            .on_action(cx.listener(|this, _: &AutoScrollSlower, _, cx| {
                this.adjust_auto_scroll_speed(-20.0, cx);
            }))
            .child(self.render_header(cx))
            .child(
                div()
                    .id(("viewport-body", self.id.0))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .cursor(cursor)
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::handle_right_mouse_down))
                    .on_mouse_down(MouseButton::Middle, cx.listener(Self::handle_middle_mouse_down))
                    .on_mouse_move(cx.listener(Self::handle_mouse_move))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
                    .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                        let delta = event.delta.pixel_delta(px(28.));
                        this.apply_scroll_delta(delta.x.as_f32(), delta.y.as_f32(), cx);
                    }))
                    .on_pinch(cx.listener(|this, event: &PinchEvent, _, cx| {
                        if event.delta.abs() > 1e-4 {
                            this.apply_zoom_factor((1.0 + event.delta).clamp(0.75, 1.35), cx);
                        }
                    }))
                    .child(canvas_el)
                    .children(hint_pills)
                    .children(mark_prompt)
                    .children(hint_banner)
                    .children(sel_popover)
                    .children(ann_popover)
                    .children(overview_popover),
            )
    }
}

fn pane_btn(
    id: (&'static str, u64),
    label: impl Into<gpui::SharedString>,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_1p5()
        .py_0p5()
        .rounded_sm()
        .bg(rgb(0xe4e4e7))
        .text_color(rgb(0x27272a))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0xd4d4d8)))
        .child(label.into())
        .on_click(on_click)
}

fn norm_rect_to_bounds(rect: NormRect, page_bounds: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(
            page_bounds.origin.x + px(rect.min_x * scale),
            page_bounds.origin.y + px(rect.min_y * scale),
        ),
        size(px(rect.width() * scale), px(rect.height() * scale)),
    )
}

fn paint_annotation_overlay(
    ann: &Annotation,
    page_bounds: Bounds<Pixels>,
    scale: f32,
    window: &mut Window,
) {
    match ann.kind {
        AnnotationKind::Highlight => {
            let color = rgba(ann.color.to_u32_rgba(Some(0.36)));
            if ann.quads.is_empty() {
                let b = norm_rect_to_bounds(ann.bounds, page_bounds, scale);
                window.paint_quad(PaintQuad::new(b).background(color).corner_radii(px(2.)));
            } else {
                for q in &ann.quads {
                    let b = norm_rect_to_bounds(q.bounding_rect(), page_bounds, scale);
                    window.paint_quad(PaintQuad::new(b).background(color).corner_radii(px(2.)));
                }
            }
        }
        AnnotationKind::Underline => {
            let color = rgba(ann.color.to_u32_rgba(Some(0.95)));
            let line_h = px((1.8 * scale).max(1.5));
            for q in &ann.quads {
                let r = q.bounding_rect();
                let b = Bounds::new(
                    point(
                        page_bounds.origin.x + px(r.min_x * scale),
                        page_bounds.origin.y + px(r.max_y * scale) - line_h,
                    ),
                    size(px(r.width() * scale), line_h),
                );
                window.paint_quad(PaintQuad::new(b).background(color));
            }
        }
        AnnotationKind::StrikeOut => {
            let color = rgba(ann.color.to_u32_rgba(Some(0.95)));
            let line_h = px((1.6 * scale).max(1.5));
            for q in &ann.quads {
                let r = q.bounding_rect();
                let mid_y = (r.min_y + r.max_y) * 0.5;
                let b = Bounds::new(
                    point(
                        page_bounds.origin.x + px(r.min_x * scale),
                        page_bounds.origin.y + px(mid_y * scale),
                    ),
                    size(px(r.width() * scale), line_h),
                );
                window.paint_quad(PaintQuad::new(b).background(color));
            }
        }
        AnnotationKind::AreaNote => {
            let b = norm_rect_to_bounds(ann.bounds, page_bounds, scale);
            window.paint_quad(
                PaintQuad::new(b)
                    .background(rgba(ann.color.to_u32_rgba(Some(0.14))))
                    .border_widths(px(2.))
                    .border_color(rgba(ann.color.to_u32_rgba(Some(0.9))))
                    .corner_radii(px(3.)),
            );
        }
        AnnotationKind::StickyNote => {
            let b = Bounds::new(
                point(
                    page_bounds.origin.x + px(ann.bounds.min_x * scale),
                    page_bounds.origin.y + px(ann.bounds.min_y * scale),
                ),
                size(px(18.), px(18.)),
            );
            window.paint_quad(
                PaintQuad::new(b)
                    .background(rgba(ann.color.to_u32_rgba(Some(0.95))))
                    .border_widths(px(1.))
                    .border_color(rgb(0x713f12))
                    .corner_radii(px(4.)),
            );
        }
    }

    // Small note indicator dot in top-right if the annotation carries a note comment.
    if ann.note_contents.as_ref().is_some_and(|s| !s.is_empty()) {
        let b = norm_rect_to_bounds(ann.bounds, page_bounds, scale);
        let dot = Bounds::new(
            point(b.origin.x + b.size.width - px(6.), b.origin.y - px(3.)),
            size(px(8.), px(8.)),
        );
        window.paint_quad(
            PaintQuad::new(dot)
                .background(rgb(0x2563eb))
                .border_widths(px(1.))
                .border_color(rgb(0xffffff))
                .corner_radii(px(4.)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generate_hint_labels_produces_unique_two_letter_codes() {
        let labels = generate_hint_labels(120);
        assert_eq!(labels.len(), 120);
        let set: HashSet<&str> = labels.iter().map(String::as_str).collect();
        assert_eq!(set.len(), 120);
        assert_eq!(labels[0], "aa");
        assert!(labels.iter().all(|l| l.len() == 2 && l.chars().all(|c| c.is_ascii_lowercase())));
    }
}
