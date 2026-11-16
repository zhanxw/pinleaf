use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, AnyWindowHandle, App, Axis, Bounds, Context, Entity, FocusHandle, FontWeight,
    KeyDownEvent, MouseButton, MouseMoveEvent, PathPromptOptions, Pixels, Render, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, canvas, div, img, prelude::*, px, rgb,
    rgba, size,
};
use pinleaf_core::{AnnotationId, AnnotationKind, FreezeState, PageIndex, ReadingAnchor, ViewportId};
use pinleaf_engine::filter_toc;

use crate::actions::{
    APP_CONTEXT, COMMAND_POPUP_CONTEXT, CycleColorMode, NextSearchMatch, OVERLAY_POPUP_CONTEXT,
    OpenDemo, OpenFile, OpenGlobalHighlightsPopup, OpenRecentPopup, OpenSearch, PrevSearchMatch,
    RedoAnnot, SEARCH_BAR_CONTEXT, SavePdf, SavePdfAs, ToggleAutoReload, ToggleCommandPopup,
    ToggleSidebar, ToggleTocPopup, UndoAnnot,
};
use crate::app_state::{AppRegistry, navigate_viewport_globally};
use crate::command_popup::{CommandEntry, CommandId, filter_commands};
use crate::library::{GlobalHighlightHit, IndexedHighlight, matches_highlight};
use crate::session::{ColorMode, DocumentSession};
use crate::viewport::{PdfViewport, ViewportEvent};

pub enum SplitNode {
    Leaf {
        viewport: Entity<PdfViewport>,
    },
    Split {
        split_id: u64,
        axis: Axis,
        ratio: f32,
        bounds_cell: Rc<Cell<Bounds<Pixels>>>,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

impl SplitNode {
    fn first_viewport(&self) -> &Entity<PdfViewport> {
        match self {
            SplitNode::Leaf { viewport } => viewport,
            SplitNode::Split { first, .. } => first.first_viewport(),
        }
    }

    fn find_viewport(&self, id: ViewportId, cx: &App) -> Option<Entity<PdfViewport>> {
        match self {
            SplitNode::Leaf { viewport } => {
                if viewport.read(cx).id == id {
                    Some(viewport.clone())
                } else {
                    None
                }
            }
            SplitNode::Split { first, second, .. } => {
                first.find_viewport(id, cx).or_else(|| second.find_viewport(id, cx))
            }
        }
    }

    fn all_viewports(&self, out: &mut Vec<Entity<PdfViewport>>) {
        match self {
            SplitNode::Leaf { viewport } => out.push(viewport.clone()),
            SplitNode::Split { first, second, .. } => {
                first.all_viewports(out);
                second.all_viewports(out);
            }
        }
    }

    fn leaf_count(&self) -> usize {
        match self {
            SplitNode::Leaf { .. } => 1,
            SplitNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    fn replace_leaf_with_split(
        &mut self,
        target_id: ViewportId,
        split_id: u64,
        axis: Axis,
        new_child: Entity<PdfViewport>,
        cx: &App,
    ) -> bool {
        match self {
            SplitNode::Leaf { viewport } => {
                if viewport.read(cx).id == target_id {
                    let existing = viewport.clone();
                    *self = SplitNode::Split {
                        split_id,
                        axis,
                        ratio: 0.5,
                        bounds_cell: Rc::new(Cell::new(Bounds::default())),
                        first: Box::new(SplitNode::Leaf { viewport: existing }),
                        second: Box::new(SplitNode::Leaf {
                            viewport: new_child,
                        }),
                    };
                    true
                } else {
                    false
                }
            }
            SplitNode::Split { first, second, .. } => {
                first.replace_leaf_with_split(target_id, split_id, axis, new_child.clone(), cx)
                    || second.replace_leaf_with_split(target_id, split_id, axis, new_child, cx)
            }
        }
    }

    fn update_split_ratio(&mut self, target_split_id: u64, new_ratio: f32) -> bool {
        match self {
            SplitNode::Leaf { .. } => false,
            SplitNode::Split {
                split_id,
                ratio,
                first,
                second,
                ..
            } => {
                if *split_id == target_split_id {
                    *ratio = new_ratio.clamp(0.15, 0.85);
                    true
                } else {
                    first.update_split_ratio(target_split_id, new_ratio)
                        || second.update_split_ratio(target_split_id, new_ratio)
                }
            }
        }
    }

    fn remove_leaf(
        node: SplitNode,
        target_id: ViewportId,
        cx: &App,
    ) -> (Option<SplitNode>, Option<Entity<PdfViewport>>) {
        match node {
            SplitNode::Leaf { viewport } => {
                if viewport.read(cx).id == target_id {
                    (None, Some(viewport))
                } else {
                    (Some(SplitNode::Leaf { viewport }), None)
                }
            }
            SplitNode::Split {
                split_id,
                axis,
                ratio,
                bounds_cell,
                first,
                second,
            } => {
                let (new_first, removed_l) = Self::remove_leaf(*first, target_id, cx);
                if removed_l.is_some() {
                    return match new_first {
                        Some(f) => (
                            Some(SplitNode::Split {
                                split_id,
                                axis,
                                ratio,
                                bounds_cell,
                                first: Box::new(f),
                                second,
                            }),
                            removed_l,
                        ),
                        None => (Some(*second), removed_l),
                    };
                }
                let (new_second, removed_r) = Self::remove_leaf(*second, target_id, cx);
                if removed_r.is_some() {
                    return match new_second {
                        Some(s) => (
                            Some(SplitNode::Split {
                                split_id,
                                axis,
                                ratio,
                                bounds_cell,
                                first: new_first.map(Box::new).expect("first unchanged"),
                                second: Box::new(s),
                            }),
                            removed_r,
                        ),
                        None => (new_first, removed_r),
                    };
                }
                (
                    Some(SplitNode::Split {
                        split_id,
                        axis,
                        ratio,
                        bounds_cell,
                        first: Box::new(new_first.expect("unchanged")),
                        second: Box::new(new_second.expect("unchanged")),
                    }),
                    None,
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LibraryModalKind {
    RecentDocs,
    GlobalHighlights,
}

#[derive(Clone)]
struct LibraryRowItem {
    title: String,
    subtitle: String,
    badge: String,
    target_path: Option<PathBuf>,
    target_page: PageIndex,
}

pub struct WindowWorkspace {
    session: Entity<DocumentSession>,
    root: SplitNode,
    focused_viewport: ViewportId,
    sidebar_open: bool,
    command_popup_open: bool,
    command_query: String,
    selected_command_idx: usize,
    command_focus_handle: FocusHandle,
    search_bar_open: bool,
    search_focus_handle: FocusHandle,
    toc_popup_open: bool,
    toc_query: String,
    toc_selected_idx: usize,
    toc_focus_handle: FocusHandle,
    library_modal: Option<LibraryModalKind>,
    library_query: String,
    library_selected_idx: usize,
    library_focus_handle: FocusHandle,
    next_split_id: u64,
    active_split_drag: Option<(u64, Axis, Rc<Cell<Bounds<Pixels>>>)>,
    subscriptions: HashMap<ViewportId, Subscription>,
    _session_obs: Subscription,
    pending_focus: Option<FocusHandle>,
    pending_close_window: bool,
    last_window_title: String,
}

impl WindowWorkspace {
    pub fn new_with_anchor(
        session: Entity<DocumentSession>,
        parent_viewport: Option<ViewportId>,
        anchor: ReadingAnchor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let win_handle = window.window_handle();
        let viewport = cx.new(|cx| {
            let mut tmp_vp = PdfViewport::new(
                ViewportId(0),
                session.clone(),
                anchor,
                FreezeState::Active,
                cx,
            );
            let node = cx.update_global::<AppRegistry, _>(|reg, cx_inner| {
                let _ = cx_inner;
                // Placeholder replaced right after cx.new completes.
                reg.trail_graph.register_child(parent_viewport, FreezeState::Active)
            });
            tmp_vp.id = node.id;
            tmp_vp
        });

        let vp_id = viewport.read(cx).id;
        cx.update_global::<AppRegistry, _>(|reg, _| {
            reg.viewports.insert(
                vp_id,
                crate::app_state::ViewportRegistryEntry {
                    viewport: viewport.downgrade(),
                    window: win_handle,
                },
            );
        });

        Self::from_existing_viewport(session, viewport, window, cx)
    }

    pub fn from_existing_viewport(
        session: Entity<DocumentSession>,
        viewport: Entity<PdfViewport>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let vp_id = viewport.read(cx).id;
        let win_handle = window.window_handle();
        cx.update_global::<AppRegistry, _>(|reg, _| {
            reg.update_viewport_window(vp_id, win_handle);
        });

        let fh = viewport.read(cx).focus_handle().clone();
        let sub = Self::subscribe_viewport(&viewport, cx);
        let mut subscriptions = HashMap::new();
        subscriptions.insert(vp_id, sub);

        let _session_obs = cx.observe(&session, |_, _, cx| cx.notify());
        session.update(cx, |s, cx| s.trigger_background_text_index(cx));

        Self {
            session,
            root: SplitNode::Leaf { viewport },
            focused_viewport: vp_id,
            sidebar_open: false,
            command_popup_open: false,
            command_query: String::new(),
            selected_command_idx: 0,
            command_focus_handle: cx.focus_handle(),
            search_bar_open: false,
            search_focus_handle: cx.focus_handle(),
            toc_popup_open: false,
            toc_query: String::new(),
            toc_selected_idx: 0,
            toc_focus_handle: cx.focus_handle(),
            library_modal: None,
            library_query: String::new(),
            library_selected_idx: 0,
            library_focus_handle: cx.focus_handle(),
            next_split_id: 1,
            active_split_drag: None,
            subscriptions,
            _session_obs,
            pending_focus: Some(fh),
            pending_close_window: false,
            last_window_title: String::new(),
        }
    }

    pub fn is_untouched_demo(&self, cx: &App) -> bool {
        if self.root.leaf_count() != 1 {
            return false;
        }
        let s = self.session.read(cx);
        s.path.is_none() && !s.annotations.is_dirty()
    }

    pub fn open_external_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.open_or_switch_path_at_page(path, PageIndex(0), cx);
    }

    fn subscribe_viewport(
        viewport: &Entity<PdfViewport>,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe(viewport, |ws, _, event, cx| match *event {
            ViewportEvent::Focused(id) => {
                ws.focused_viewport = id;
                cx.notify();
            }
            ViewportEvent::RequestFreezeAndSplit { id, axis } => {
                ws.freeze_and_split(id, axis, cx);
            }
            ViewportEvent::RequestFreezeAndNewWindow(id) => {
                ws.freeze_and_open_window(id, cx);
            }
            ViewportEvent::RequestDetachToWindow(id) => {
                ws.detach_pane_to_window(id, cx);
            }
            ViewportEvent::RequestClose(id) => {
                ws.close_pane(id, cx);
            }
            ViewportEvent::RequestSmartLinkRoute {
                source_id,
                page,
                top_norm_y,
            } => {
                ws.smart_route_link(source_id, page, top_norm_y, cx);
            }
        })
    }

    fn spawn_child_viewport(
        &mut self,
        parent_id: ViewportId,
        anchor: ReadingAnchor,
        window_handle: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Entity<PdfViewport> {
        let session = self.session.clone();
        let child = cx.new(|cx| {
            PdfViewport::new(
                ViewportId(0),
                session,
                anchor,
                FreezeState::Active,
                cx,
            )
        });
        let node = cx.update_global::<AppRegistry, _>(|reg, _| {
            reg.register_viewport(Some(parent_id), FreezeState::Active, &child, window_handle)
        });
        child.update(cx, |vp, _| {
            vp.id = node.id;
        });
        let sub = Self::subscribe_viewport(&child, cx);
        self.subscriptions.insert(node.id, sub);
        child
    }

    fn freeze_and_split(
        &mut self,
        target_id: ViewportId,
        axis: Axis,
        cx: &mut Context<Self>,
    ) -> Option<Entity<PdfViewport>> {
        let target_vp = self.root.find_viewport(target_id, cx)?;
        let prev_anchor = target_vp.read(cx).anchor;
        let window_handle = cx
            .global::<AppRegistry>()
            .viewports
            .get(&target_id)
            .map(|e| e.window)?;

        target_vp.update(cx, |vp, cx| {
            vp.freeze_with_locked_scale(cx);
        });

        let child_anchor = ReadingAnchor {
            page: prev_anchor.page,
            focal_norm: prev_anchor.focal_norm,
            zoom: prev_anchor.zoom,
        };

        let child = self.spawn_child_viewport(target_id, child_anchor, window_handle, cx);
        let child_id = child.read(cx).id;
        let child_fh = child.read(cx).focus_handle().clone();

        let split_id = self.next_split_id;
        self.next_split_id += 1;
        self.root
            .replace_leaf_with_split(target_id, split_id, axis, child.clone(), cx);
        self.focused_viewport = child_id;
        self.pending_focus = Some(child_fh);
        cx.notify();
        Some(child)
    }

    fn freeze_and_open_window(&mut self, target_id: ViewportId, cx: &mut Context<Self>) {
        let Some(target_vp) = self.root.find_viewport(target_id, cx) else {
            return;
        };
        let prev_anchor = target_vp.read(cx).anchor;
        target_vp.update(cx, |vp, cx| {
            vp.freeze_with_locked_scale(cx);
        });

        let child_anchor = ReadingAnchor {
            page: prev_anchor.page,
            focal_norm: prev_anchor.focal_norm,
            zoom: prev_anchor.zoom,
        };
        open_workspace_window(self.session.clone(), Some(target_id), child_anchor, cx);
    }

    fn detach_pane_to_window(&mut self, target_id: ViewportId, cx: &mut Context<Self>) {
        if self.root.leaf_count() <= 1 {
            return;
        }
        let dummy_root = SplitNode::Leaf {
            viewport: self.root.first_viewport().clone(),
        };
        let old_root = std::mem::replace(&mut self.root, dummy_root);
        let (remaining, detached) = SplitNode::remove_leaf(old_root, target_id, cx);
        if let Some(rem) = remaining {
            self.root = rem;
        }
        let Some(detached_vp) = detached else {
            return;
        };

        self.subscriptions.remove(&target_id);
        let next_focus_vp = self.root.first_viewport().clone();
        self.focused_viewport = next_focus_vp.read(cx).id;
        self.pending_focus = Some(next_focus_vp.read(cx).focus_handle().clone());

        let session = self.session.clone();
        let bounds = Bounds::centered(None, size(px(900.), px(780.)), cx);
        let title = format!("Pinleaf — {} (Detached)", session.read(cx).title);
        let _ = cx.open_window(
            WindowOptions::default()
                .window_bounds(WindowBounds::Windowed(bounds))
                .titlebar(Some(TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                })),
            move |window, cx| {
                cx.new(|cx| WindowWorkspace::from_existing_viewport(session, detached_vp, window, cx))
            },
        );
        cx.activate(true);
        cx.notify();
    }

    fn close_pane(&mut self, target_id: ViewportId, cx: &mut Context<Self>) {
        if self.root.leaf_count() <= 1 {
            cx.update_global::<AppRegistry, _>(|reg, _| {
                reg.unregister_viewport(target_id);
            });
            self.pending_close_window = true;
            cx.notify();
            return;
        }

        let dummy_root = SplitNode::Leaf {
            viewport: self.root.first_viewport().clone(),
        };
        let old_root = std::mem::replace(&mut self.root, dummy_root);
        let (remaining, removed) = SplitNode::remove_leaf(old_root, target_id, cx);
        if let Some(rem) = remaining {
            self.root = rem;
        }
        if removed.is_some() {
            self.subscriptions.remove(&target_id);
            cx.update_global::<AppRegistry, _>(|reg, _| {
                reg.unregister_viewport(target_id);
            });
            let next_vp = self.root.first_viewport().clone();
            self.focused_viewport = next_vp.read(cx).id;
            self.pending_focus = Some(next_vp.read(cx).focus_handle().clone());
        }
        cx.notify();
    }

    fn smart_route_link(
        &mut self,
        source_id: ViewportId,
        page: PageIndex,
        top_norm_y: Option<f32>,
        cx: &mut Context<Self>,
    ) {
        if let Some(target_id) = cx.global::<AppRegistry>().find_smart_route_target(source_id)
            && navigate_viewport_globally(target_id, page, top_norm_y, cx)
        {
            return;
        }
        if let Some(child) = self.freeze_and_split(source_id, Axis::Horizontal, cx) {
            child.update(cx, |vp, cx| {
                vp.navigate_to_page_offset(page, top_norm_y.unwrap_or(0.0), cx);
            });
        }
    }

    fn active_viewport_entity(&self, cx: &App) -> Entity<PdfViewport> {
        self.root
            .find_viewport(self.focused_viewport, cx)
            .unwrap_or_else(|| self.root.first_viewport().clone())
    }

    fn open_demo_session(&mut self, cx: &mut Context<Self>) {
        if let Ok(demo) = DocumentSession::open_demo() {
            let entity = cx.new(|_| demo);
            cx.update_global::<AppRegistry, _>(|reg, _| {
                reg.register_session(None, &entity);
            });
            self.switch_session(entity, cx);
        }
    }

    fn open_command_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_popup_open = true;
        self.command_query.clear();
        self.selected_command_idx = 0;
        window.focus(&self.command_focus_handle, cx);
        cx.notify();
    }

    fn close_command_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_popup_open = false;
        self.command_query.clear();
        self.selected_command_idx = 0;
        let fh = self.active_viewport_entity(cx).read(cx).focus_handle().clone();
        window.focus(&fh, cx);
        cx.notify();
    }

    fn toggle_command_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_popup_open {
            self.close_command_popup(window, cx);
        } else {
            self.open_command_popup(window, cx);
        }
    }

    fn navigate_focused_or_smart_route(
        &mut self,
        page: PageIndex,
        top_norm_y: f32,
        cx: &mut Context<Self>,
    ) {
        let focused_vp = self.active_viewport_entity(cx);
        let is_frozen = focused_vp.read(cx).freeze_state.is_frozen();
        if !is_frozen {
            focused_vp.update(cx, |vp, cx| {
                vp.jump_to_page_offset_recorded(page, top_norm_y, cx);
            });
            return;
        }
        let source_id = focused_vp.read(cx).id;
        if let Some(target_id) = cx.global::<AppRegistry>().find_smart_route_target(source_id) {
            navigate_viewport_globally(target_id, page, Some(top_norm_y), cx);
            return;
        }
        if let Some(child) = self.freeze_and_split(source_id, Axis::Horizontal, cx) {
            child.update(cx, |vp, cx| {
                vp.jump_to_page_offset_recorded(page, top_norm_y, cx);
            });
        }
    }

    fn open_search_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_popup_open = false;
        self.toc_popup_open = false;
        self.search_bar_open = true;
        window.focus(&self.search_focus_handle, cx);
        self.session.update(cx, |s, cx| s.trigger_background_text_index(cx));
        cx.notify();
    }

    fn close_search_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_bar_open = false;
        self.session.update(cx, |s, cx| s.clear_search(cx));
        let fh = self.active_viewport_entity(cx).read(cx).focus_handle().clone();
        window.focus(&fh, cx);
        cx.notify();
    }

    fn step_search(&mut self, delta: i32, cx: &mut Context<Self>) {
        let target = self
            .session
            .update(cx, |s, cx| s.step_search_match(delta, cx));
        if let Some((page, y)) = target {
            self.navigate_focused_or_smart_route(page, (y - 36.0).max(0.0), cx);
        }
    }

    fn open_toc_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_popup_open = false;
        self.toc_popup_open = true;
        self.toc_query.clear();
        self.toc_selected_idx = 0;
        window.focus(&self.toc_focus_handle, cx);
        cx.notify();
    }

    fn close_toc_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.toc_popup_open = false;
        self.toc_query.clear();
        self.toc_selected_idx = 0;
        let fh = self.active_viewport_entity(cx).read(cx).focus_handle().clone();
        window.focus(&fh, cx);
        cx.notify();
    }

    fn toggle_toc_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.toc_popup_open {
            self.close_toc_popup(window, cx);
        } else {
            self.open_toc_popup(window, cx);
        }
    }

    fn jump_to_toc_entry(
        &mut self,
        page: PageIndex,
        top_norm_y: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_toc_popup(window, cx);
        self.navigate_focused_or_smart_route(page, (top_norm_y - 24.0).max(0.0), cx);
    }

    fn execute_command(&mut self, cmd: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        // Close popup and restore viewport focus first so commands that open a new
        // split pane or focus the inline note editor keep their new focus target.
        self.close_command_popup(window, cx);

        let vp = self.active_viewport_entity(cx);
        let vp_id = vp.read(cx).id;

        match cmd {
            CommandId::ToggleFreeze => vp.update(cx, |vp, cx| vp.toggle_freeze(cx)),
            CommandId::FreezeAndSplitRight => {
                self.freeze_and_split(vp_id, Axis::Horizontal, cx);
            }
            CommandId::FreezeAndSplitDown => {
                self.freeze_and_split(vp_id, Axis::Vertical, cx);
            }
            CommandId::FreezeAndNewWindow => self.freeze_and_open_window(vp_id, cx),
            CommandId::DetachPaneToWindow => self.detach_pane_to_window(vp_id, cx),
            CommandId::ClosePane => self.close_pane(vp_id, cx),
            CommandId::HighlightSelection => {
                vp.update(cx, |vp, cx| vp.highlight_selection(window, cx));
            }
            CommandId::AnnotateWithNote => {
                vp.update(cx, |vp, cx| vp.annotate_selection_with_note(window, cx));
            }
            CommandId::UnderlineSelection => {
                vp.update(cx, |vp, cx| vp.underline_selection(window, cx));
            }
            CommandId::StrikeOutSelection => {
                vp.update(cx, |vp, cx| vp.strikeout_selection(window, cx));
            }
            CommandId::ToggleAreaMode => vp.update(cx, |vp, cx| vp.toggle_area_mode(cx)),
            CommandId::CopySelection => vp.update(cx, |vp, cx| vp.copy_selection(cx)),
            CommandId::ToggleSidebar => {
                self.sidebar_open = !self.sidebar_open;
                cx.notify();
            }
            CommandId::UndoAnnot => self.session.update(cx, |s, cx| s.undo(cx)),
            CommandId::RedoAnnot => self.session.update(cx, |s, cx| s.redo(cx)),
            CommandId::NextPage => vp.update(cx, |vp, cx| vp.next_page(cx)),
            CommandId::PrevPage => vp.update(cx, |vp, cx| vp.prev_page(cx)),
            CommandId::JumpBack => vp.update(cx, |vp, cx| vp.go_back(cx)),
            CommandId::JumpForward => vp.update(cx, |vp, cx| vp.go_forward(cx)),
            CommandId::FirstPage => vp.update(cx, |vp, cx| vp.go_first_page(cx)),
            CommandId::LastPage => vp.update(cx, |vp, cx| vp.go_last_page(cx)),
            CommandId::OpenSearch => self.open_search_bar(window, cx),
            CommandId::NextSearchMatch => self.step_search(1, cx),
            CommandId::PrevSearchMatch => self.step_search(-1, cx),
            CommandId::ToggleTocPopup => self.toggle_toc_popup(window, cx),
            CommandId::BeginSetMark => vp.update(cx, |vp, cx| vp.begin_set_mark(cx)),
            CommandId::BeginJumpMark => vp.update(cx, |vp, cx| vp.begin_jump_mark(cx)),
            CommandId::ZoomIn => vp.update(cx, |vp, cx| vp.zoom_in(cx)),
            CommandId::ZoomOut => vp.update(cx, |vp, cx| vp.zoom_out(cx)),
            CommandId::ZoomFitWidth => vp.update(cx, |vp, cx| vp.zoom_fit_width(cx)),
            CommandId::ZoomFitContentWidth => {
                vp.update(cx, |vp, cx| vp.zoom_fit_content_width(cx));
            }
            CommandId::CycleColorMode => self.session.update(cx, |s, cx| s.cycle_color_mode(cx)),
            CommandId::ToggleReadingRuler => vp.update(cx, |vp, cx| vp.toggle_reading_ruler(cx)),
            CommandId::LookupScholar => vp.update(cx, |vp, cx| vp.lookup_selection_scholar(cx)),
            CommandId::OpenRecentPopup => {
                self.open_library_modal(LibraryModalKind::RecentDocs, window, cx);
            }
            CommandId::OpenGlobalHighlightsPopup => {
                self.open_library_modal(LibraryModalKind::GlobalHighlights, window, cx);
            }
            CommandId::ToggleAutoReload => {
                self.session.update(cx, |s, cx| s.toggle_auto_reload(cx));
            }
            CommandId::NewWindow => cx.dispatch_action(&crate::actions::NewWindow),
            CommandId::OpenFile => self.prompt_open_file(cx),
            CommandId::OpenDemo => self.open_demo_session(cx),
            CommandId::SavePdf => self.trigger_save(false, cx),
            CommandId::SavePdfAs => self.trigger_save(true, cx),
            CommandId::QuitApp => cx.quit(),
            CommandId::CloseCommandPopup => {}
        }
    }

    fn handle_command_popup_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let ks = &event.keystroke;

        if ks.key == "escape" {
            self.close_command_popup(window, cx);
            return;
        }

        if self.command_query.is_empty()
            && (ks.key == "/"
                || ks.key == "?"
                || ks.key_char.as_deref() == Some("/")
                || ks.key_char.as_deref() == Some("?"))
        {
            self.close_command_popup(window, cx);
            return;
        }

        let matches = filter_commands(&self.command_query);
        let match_len = matches.len();

        if ks.key == "down"
            || (ks.modifiers.control && ks.key == "n")
            || (ks.key == "tab" && !ks.modifiers.shift)
        {
            if match_len > 0 {
                let current = self.selected_command_idx.min(match_len - 1);
                self.selected_command_idx = (current + 1) % match_len;
                cx.notify();
            }
            return;
        }

        if ks.key == "up"
            || (ks.modifiers.control && ks.key == "p")
            || (ks.key == "tab" && ks.modifiers.shift)
        {
            if match_len > 0 {
                let current = self.selected_command_idx.min(match_len - 1);
                self.selected_command_idx = (current + match_len - 1) % match_len;
                cx.notify();
            }
            return;
        }

        if ks.key == "enter" {
            if match_len > 0 {
                let idx = self.selected_command_idx.min(match_len - 1);
                let cmd_id = matches[idx].id;
                self.execute_command(cmd_id, window, cx);
            }
            return;
        }

        if apply_text_edit_keystroke(&mut self.command_query, event, cx) {
            self.selected_command_idx = 0;
            cx.notify();
        }
    }

    fn handle_search_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let ks = &event.keystroke;
        if ks.key == "escape" {
            self.close_search_bar(window, cx);
            return;
        }
        if ks.key == "enter" {
            let delta = if ks.modifiers.shift { -1 } else { 1 };
            self.step_search(delta, cx);
            return;
        }
        let (mut query, regex_mode) = {
            let s = &self.session.read(cx).search;
            (s.query.clone(), s.regex_mode)
        };
        if apply_text_edit_keystroke(&mut query, event, cx) {
            self.session.update(cx, |s, cx| {
                s.update_search(query, regex_mode, cx);
            });
            if let Some((page, y)) = self.session.read(cx).current_search_match() {
                self.navigate_focused_or_smart_route(page, (y - 36.0).max(0.0), cx);
            }
        }
    }

    fn handle_toc_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let ks = &event.keystroke;
        if ks.key == "escape" {
            self.close_toc_popup(window, cx);
            return;
        }

        let toc_snapshot = self.session.read(cx).toc_entries.clone();
        let filtered = filter_toc(&toc_snapshot, &self.toc_query);
        let len = filtered.len();

        if ks.key == "down"
            || (ks.modifiers.control && ks.key == "n")
            || (ks.key == "tab" && !ks.modifiers.shift)
        {
            if len > 0 {
                let current = self.toc_selected_idx.min(len - 1);
                self.toc_selected_idx = (current + 1) % len;
                cx.notify();
            }
            return;
        }
        if ks.key == "up"
            || (ks.modifiers.control && ks.key == "p")
            || (ks.key == "tab" && ks.modifiers.shift)
        {
            if len > 0 {
                let current = self.toc_selected_idx.min(len - 1);
                self.toc_selected_idx = (current + len - 1) % len;
                cx.notify();
            }
            return;
        }
        if ks.key == "enter" {
            if let Some(entry) = filtered.get(self.toc_selected_idx.min(len.saturating_sub(1))) {
                self.jump_to_toc_entry(entry.page, entry.top_norm_y, window, cx);
            }
            return;
        }

        if apply_text_edit_keystroke(&mut self.toc_query, event, cx) {
            self.toc_selected_idx = 0;
            cx.notify();
        }
    }

    fn sync_current_session_library(&self, cx: &mut Context<Self>) {
        let last_page = self.active_viewport_entity(cx).read(cx).anchor.page.0;
        let session_entity = self.session.clone();
        cx.update_global::<AppRegistry, _>(|reg, cx_inner| {
            let s = session_entity.read(cx_inner);
            reg.sync_session_to_library(s, last_page);
        });
    }

    fn open_library_modal(
        &mut self,
        kind: LibraryModalKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_current_session_library(cx);
        self.command_popup_open = false;
        self.toc_popup_open = false;
        self.library_modal = Some(kind);
        self.library_query.clear();
        self.library_selected_idx = 0;
        window.focus(&self.library_focus_handle, cx);
        cx.notify();
    }

    fn close_library_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.library_modal = None;
        self.library_query.clear();
        self.library_selected_idx = 0;
        let fh = self.active_viewport_entity(cx).read(cx).focus_handle().clone();
        window.focus(&fh, cx);
        cx.notify();
    }

    fn toggle_library_modal(
        &mut self,
        kind: LibraryModalKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.library_modal == Some(kind) {
            self.close_library_modal(window, cx);
        } else {
            self.open_library_modal(kind, window, cx);
        }
    }

    fn open_or_switch_path_at_page(
        &mut self,
        path: PathBuf,
        target_page: PageIndex,
        cx: &mut Context<Self>,
    ) {
        let same_as_current = self
            .session
            .read(cx)
            .path
            .as_ref()
            .is_some_and(|p| p == &path || std::fs::canonicalize(p).ok() == std::fs::canonicalize(&path).ok());
        if same_as_current {
            self.navigate_focused_or_smart_route(target_page, 0.0, cx);
            return;
        }
        if let Some(existing) = cx.global::<AppRegistry>().lookup_session(&path) {
            self.switch_session(existing, cx);
            self.navigate_focused_or_smart_route(target_page, 0.0, cx);
            return;
        }
        match DocumentSession::open_path(path.clone()) {
            Ok(mut session_model) => {
                cx.global::<AppRegistry>()
                    .prepare_session_model(&mut session_model);
                let entity = cx.new(|_| session_model);
                AppRegistry::register_and_start_session(&entity, target_page.0, cx);
                self.switch_session(entity, cx);
                self.navigate_focused_or_smart_route(target_page, 0.0, cx);
            }
            Err(err) => {
                self.session.update(cx, |s, cx| {
                    s.status_message = Some((format!("Open failed: {err}"), true));
                    cx.notify();
                });
            }
        }
    }

    fn collect_library_items(&self, kind: LibraryModalKind, cx: &App) -> Vec<LibraryRowItem> {
        let reg = cx.global::<AppRegistry>();
        match kind {
            LibraryModalKind::RecentDocs => reg
                .library
                .filter_recent(&self.library_query)
                .into_iter()
                .map(|doc| LibraryRowItem {
                    title: doc.title.clone(),
                    subtitle: format!(
                        "{} — {} pages · {} notes",
                        doc.path.display(),
                        doc.page_count,
                        doc.highlights.len()
                    ),
                    badge: format!("p.{}", doc.last_page + 1),
                    target_path: Some(doc.path.clone()),
                    target_page: PageIndex(doc.last_page),
                })
                .collect(),
            LibraryModalKind::GlobalHighlights => {
                let s = self.session.read(cx);
                let cur_canon = s
                    .path
                    .as_ref()
                    .and_then(|p| std::fs::canonicalize(p).ok().or_else(|| Some(p.clone())));
                let tokens: Vec<String> = self
                    .library_query
                    .split_whitespace()
                    .map(|t| t.to_ascii_lowercase())
                    .collect();

                let mut hits: Vec<GlobalHighlightHit> = Vec::new();
                for ann in s.annotations.all_sorted() {
                    let hl = IndexedHighlight {
                        page_one_based: ann.page.one_based(),
                        quote: ann.selected_text.clone().unwrap_or_default(),
                        note: ann.note_contents.clone().unwrap_or_default(),
                    };
                    if hl.quote.is_empty() && hl.note.is_empty() {
                        continue;
                    }
                    if tokens.is_empty() || matches_highlight(&s.title, &hl, &tokens) {
                        hits.push(GlobalHighlightHit {
                            path: cur_canon.clone(),
                            doc_title: s.title.clone(),
                            page_one_based: hl.page_one_based,
                            quote: hl.quote,
                            note: hl.note,
                        });
                    }
                }
                for hit in reg.library.search_highlights(&self.library_query) {
                    if cur_canon.is_some() && hit.path == cur_canon {
                        continue;
                    }
                    hits.push(hit);
                }

                hits.into_iter()
                    .map(|h| {
                        let summary = if !h.note.is_empty() && !h.quote.is_empty() {
                            format!("{} — \"{}\"", h.note, truncate_str(&h.quote, 56))
                        } else if !h.quote.is_empty() {
                            truncate_str(&h.quote, 90)
                        } else {
                            h.note
                        };
                        LibraryRowItem {
                            title: h.doc_title,
                            subtitle: summary,
                            badge: format!("p.{}", h.page_one_based),
                            target_path: h.path,
                            target_page: PageIndex(h.page_one_based.saturating_sub(1)),
                        }
                    })
                    .collect()
            }
        }
    }

    fn activate_library_item(
        &mut self,
        target_path: Option<PathBuf>,
        target_page: PageIndex,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_library_modal(window, cx);
        if let Some(path) = target_path {
            self.open_or_switch_path_at_page(path, target_page, cx);
        } else {
            self.navigate_focused_or_smart_route(target_page, 0.0, cx);
        }
    }

    fn handle_library_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let Some(kind) = self.library_modal else {
            return;
        };
        let ks = &event.keystroke;
        if ks.key == "escape" {
            self.close_library_modal(window, cx);
            return;
        }
        let items = self.collect_library_items(kind, cx);
        let len = items.len();
        if ks.key == "down"
            || (ks.modifiers.control && ks.key == "n")
            || (ks.key == "tab" && !ks.modifiers.shift)
        {
            if len > 0 {
                let current = self.library_selected_idx.min(len - 1);
                self.library_selected_idx = (current + 1) % len;
                cx.notify();
            }
            return;
        }
        if ks.key == "up"
            || (ks.modifiers.control && ks.key == "p")
            || (ks.key == "tab" && ks.modifiers.shift)
        {
            if len > 0 {
                let current = self.library_selected_idx.min(len - 1);
                self.library_selected_idx = (current + len - 1) % len;
                cx.notify();
            }
            return;
        }
        if ks.key == "enter" {
            if let Some(item) = items.get(self.library_selected_idx.min(len.saturating_sub(1))) {
                self.activate_library_item(item.target_path.clone(), item.target_page, window, cx);
            }
            return;
        }
        if apply_text_edit_keystroke(&mut self.library_query, event, cx) {
            self.library_selected_idx = 0;
            cx.notify();
        }
    }

    fn switch_session(&mut self, new_session: Entity<DocumentSession>, cx: &mut Context<Self>) {
        AppRegistry::register_and_start_session(&new_session, 0, cx);
        let mut all_vps = Vec::new();
        self.root.all_viewports(&mut all_vps);
        for vp in all_vps {
            vp.update(cx, |vp, cx| vp.set_session(new_session.clone(), cx));
        }
        self._session_obs = cx.observe(&new_session, |_, _, cx| cx.notify());
        self.session = new_session;
        cx.notify();
    }

    fn prompt_open_file(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select PDF Document".into()),
        });

        cx.spawn(async move |ws, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = ws.update(cx, |ws, cx| {
                if let Some(existing) = cx.global::<AppRegistry>().lookup_session(&path) {
                    ws.switch_session(existing, cx);
                    return;
                }
                match DocumentSession::open_path(path.clone()) {
                    Ok(mut session_model) => {
                        cx.global::<AppRegistry>()
                            .prepare_session_model(&mut session_model);
                        let entity = cx.new(|_| session_model);
                        AppRegistry::register_and_start_session(&entity, 0, cx);
                        ws.switch_session(entity, cx);
                    }
                    Err(err) => {
                        ws.session.update(cx, |s, cx| {
                            s.status_message = Some((format!("Open failed: {err}"), true));
                            cx.notify();
                        });
                    }
                }
            });
        })
        .detach();
    }

    fn trigger_save(&mut self, force_save_as: bool, cx: &mut Context<Self>) {
        if !force_save_as {
            let saved = self.session.update(cx, |s, cx| s.save_to_current_path(cx));
            if saved {
                self.sync_current_session_library(cx);
                return;
            }
        }

        let (dir, suggested) = {
            let s = self.session.read(cx);
            let dir = s
                .path
                .as_ref()
                .and_then(|p| p.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let name = s
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("annotated-demo.pdf")
                .to_string();
            (dir, name)
        };

        let rx = cx.prompt_for_new_path(&dir, Some(&suggested));
        let session = self.session.clone();
        cx.spawn(async move |ws, cx| {
            let Ok(Ok(Some(target_path))) = rx.await else {
                return;
            };
            let _ = ws.update(cx, |ws, cx| {
                session.update(cx, |s, cx| {
                    s.save_to_path(&target_path, cx);
                });
                AppRegistry::register_and_start_session(
                    &session,
                    ws.active_viewport_entity(cx).read(cx).anchor.page.0,
                    cx,
                );
            });
        })
        .detach();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let session = self.session.read(cx);
        let dirty = session.annotations.is_dirty();
        let can_undo = session.annotations.can_undo();
        let can_redo = session.annotations.can_redo();
        let ann_count = session.annotations.all_sorted().len();
        let title = if dirty {
            format!("{} *", session.title)
        } else {
            session.title.clone()
        };
        let status = session.status_message.clone();
        let color_mode = session.color_mode;
        let app_icon = cx.global::<AppRegistry>().app_icon.clone();

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_1p5()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xf4f4f5))
            .text_xs()
            .border_b_1()
            .border_color(rgb(0x27272a))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x1e3a8a))
                            .text_color(rgb(0xe0f2fe))
                            .font_weight(FontWeight::BOLD)
                            .child(img(app_icon).size(px(18.)))
                            .child("Pinleaf"),
                    )
                    .child(toolbar_btn(
                        "open-pdf",
                        "Open PDF",
                        false,
                        cx.listener(|this, _, _, cx| this.prompt_open_file(cx)),
                    ))
                    .child(toolbar_btn(
                        "open-demo",
                        "Demo Paper",
                        false,
                        cx.listener(|this, _, _, cx| this.open_demo_session(cx)),
                    ))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if dirty { rgb(0xfbbf24) } else { rgb(0xf4f4f5) })
                            .child(title),
                    )
                    .children(status.map(|(msg, is_err)| {
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(if is_err { rgb(0x7f1d1d) } else { rgb(0x14532d) })
                            .text_color(rgb(0xffffff))
                            .child(msg)
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(toolbar_btn(
                        "undo-btn",
                        if can_undo { "Undo" } else { "Undo" },
                        false,
                        cx.listener(|this, _, _, cx| {
                            this.session.update(cx, |s, cx| s.undo(cx));
                        }),
                    ))
                    .child(toolbar_btn(
                        "redo-btn",
                        if can_redo { "Redo" } else { "Redo" },
                        false,
                        cx.listener(|this, _, _, cx| {
                            this.session.update(cx, |s, cx| s.redo(cx));
                        }),
                    ))
                    .child(toolbar_btn(
                        "save-btn",
                        if dirty { "Save PDF *" } else { "Save PDF" },
                        dirty,
                        cx.listener(|this, _, _, cx| this.trigger_save(false, cx)),
                    ))
                    .child(toolbar_btn(
                        "save-as-btn",
                        "Save As...",
                        false,
                        cx.listener(|this, _, _, cx| this.trigger_save(true, cx)),
                    ))
                    .child(toolbar_btn(
                        "recent-btn",
                        "Recent",
                        self.library_modal == Some(LibraryModalKind::RecentDocs),
                        cx.listener(|this, _, window, cx| {
                            this.toggle_library_modal(LibraryModalKind::RecentDocs, window, cx);
                        }),
                    ))
                    .child(toolbar_btn(
                        "toc-btn",
                        "TOC",
                        self.toc_popup_open,
                        cx.listener(|this, _, window, cx| this.toggle_toc_popup(window, cx)),
                    ))
                    .child(toolbar_btn(
                        "find-btn",
                        "Find",
                        self.search_bar_open,
                        cx.listener(|this, _, window, cx| this.open_search_bar(window, cx)),
                    ))
                    .child(toolbar_btn(
                        "sidebar-btn",
                        format!("Notes ({ann_count})"),
                        self.sidebar_open,
                        cx.listener(|this, _, _, cx| {
                            this.sidebar_open = !this.sidebar_open;
                            cx.notify();
                        }),
                    ))
                    .child(toolbar_btn(
                        "theme-btn",
                        format!("Theme: {}", color_mode.label()),
                        color_mode != ColorMode::Light,
                        cx.listener(|this, _, _, cx| {
                            this.session.update(cx, |s, cx| s.cycle_color_mode(cx));
                        }),
                    ))
                    .child(toolbar_btn(
                        "commands-btn",
                        "? Commands",
                        self.command_popup_open,
                        cx.listener(|this, _, window, cx| this.toggle_command_popup(window, cx)),
                    )),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self
            .session
            .read(cx)
            .annotations
            .all_sorted()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        div()
            .w(px(270.))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0xfafafa))
            .border_l_1()
            .border_color(rgb(0xd4d4d8))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0xe4e4e7))
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x18181b))
                    .child(format!("Annotations ({})", items.len())),
            )
            .child(
                div()
                    .id("sidebar-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(items.is_empty(), |col| {
                        col.child(
                            div()
                                .p_3()
                                .text_xs()
                                .text_color(rgb(0x71717a))
                                .child("Drag over text to highlight, or Alt+Drag over diagrams to add Area Notes."),
                        )
                    })
                    .children(items.into_iter().enumerate().map(|(idx, ann)| {
                        let kind_label = match ann.kind {
                            AnnotationKind::Highlight => "Highlight",
                            AnnotationKind::Underline => "Underline",
                            AnnotationKind::StrikeOut => "StrikeOut",
                            AnnotationKind::AreaNote => "Area",
                            AnnotationKind::StickyNote => "Note",
                        };
                        let page_num = ann.page.one_based();
                        let quote = ann.selected_text.clone().unwrap_or_default();
                        let note = ann.note_contents.clone().unwrap_or_default();
                        let dot_color = rgba(ann.color.to_u32_rgba(Some(1.0)));
                        let ann_id = ann.id;
                        let target_page = ann.page;
                        let target_y = ann.bounds.min_y;

                        div()
                            .id(("sb-ann", idx))
                            .p_2()
                            .rounded_md()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgb(0xe4e4e7))
                            .cursor_pointer()
                            .hover(|s| s.border_color(rgb(0x2563eb)))
                            .on_click(cx.listener(move |ws, _, window, cx| {
                                ws.jump_to_annotation(ann_id, target_page, target_y, window, cx);
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_xs()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(div().size(px(10.)).rounded_full().bg(dot_color))
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0x27272a))
                                                    .child(kind_label),
                                            ),
                                    )
                                    .child(div().text_color(rgb(0x71717a)).child(format!("p.{page_num}"))),
                            )
                            .when(!quote.is_empty(), |card| {
                                card.child(
                                    div()
                                        .mt_1()
                                        .text_xs()
                                        .text_color(rgb(0x52525b))
                                        .child(truncate_str(&quote, 90)),
                                )
                            })
                            .when(!note.is_empty(), |card| {
                                card.child(
                                    div()
                                        .mt_1()
                                        .px_1p5()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(rgb(0xeff6ff))
                                        .text_xs()
                                        .text_color(rgb(0x1e3a8a))
                                        .child(note),
                                )
                            })
                    })),
            )
    }

    fn jump_to_annotation(
        &mut self,
        ann_id: AnnotationId,
        page: PageIndex,
        norm_y: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate_focused_or_smart_route(page, (norm_y - 30.0).max(0.0), cx);
        let target_vp = self.active_viewport_entity(cx);
        target_vp.update(cx, |vp, cx| {
            vp.inspect_annotation(ann_id, false, window, cx);
        });
    }

    fn render_split_node(&self, node: &SplitNode, cx: &mut Context<Self>) -> AnyElement {
        match node {
            SplitNode::Leaf { viewport } => div()
                .size_full()
                .min_w_0()
                .min_h_0()
                .child(viewport.clone())
                .into_any_element(),
            SplitNode::Split {
                split_id,
                axis,
                ratio,
                bounds_cell,
                first,
                second,
            } => {
                let first_el = self.render_split_node(first, cx);
                let second_el = self.render_split_node(second, cx);
                let split_id_val = *split_id;
                let axis_val = *axis;
                let bounds_cell_for_canvas = bounds_cell.clone();
                let bounds_cell_for_drag = bounds_cell.clone();
                let r = *ratio;

                let divider = div()
                    .id(("split-divider", split_id_val))
                    .flex_shrink_0()
                    .bg(rgb(0xd4d4d8))
                    .hover(|s| s.bg(rgb(0x2563eb)))
                    .map(|d| match axis_val {
                        Axis::Horizontal => d.w(px(5.)).h_full().cursor_col_resize(),
                        Axis::Vertical => d.h(px(5.)).w_full().cursor_row_resize(),
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, _, _, cx| {
                            ws.active_split_drag =
                                Some((split_id_val, axis_val, bounds_cell_for_drag.clone()));
                            cx.stop_propagation();
                        }),
                    );

                let bounds_tracker = canvas(
                    move |b, _, _| {
                        bounds_cell_for_canvas.set(b);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full();

                let container = match axis_val {
                    Axis::Horizontal => div().size_full().flex().flex_row().min_w_0().min_h_0(),
                    Axis::Vertical => div().size_full().flex().flex_col().min_w_0().min_h_0(),
                };

                container
                    .relative()
                    .child(bounds_tracker)
                    .child(
                        div()
                            .flex_grow(r)
                            .flex_basis(px(0.))
                            .min_w_0()
                            .min_h_0()
                            .child(first_el),
                    )
                    .child(divider)
                    .child(
                        div()
                            .flex_grow(1.0 - r)
                            .flex_basis(px(0.))
                            .min_w_0()
                            .min_h_0()
                            .child(second_el),
                    )
                    .into_any_element()
            }
        }
    }

    fn render_command_popup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let matches = filter_commands(&self.command_query);
        let match_len = matches.len();
        let selected_idx = if match_len > 0 {
            Some(self.selected_command_idx.min(match_len - 1))
        } else {
            None
        };
        let is_unfiltered = self.command_query.trim().is_empty();

        let mut list_items: Vec<AnyElement> = Vec::new();
        if matches.is_empty() {
            list_items.push(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0xa1a1aa))
                    .child(format!(
                        "No matching commands for \"{}\".",
                        self.command_query.trim()
                    ))
                    .into_any_element(),
            );
        } else {
            let mut last_category: Option<&'static str> = None;
            for (idx, entry) in matches.into_iter().enumerate() {
                if is_unfiltered && last_category != Some(entry.category) {
                    last_category = Some(entry.category);
                    list_items.push(
                        div()
                            .px_2()
                            .pt_2()
                            .pb_0p5()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x93c5fd))
                            .child(entry.category)
                            .into_any_element(),
                    );
                }
                list_items.push(self.render_command_row(
                    idx,
                    entry,
                    selected_idx == Some(idx),
                    !is_unfiltered,
                    cx,
                ));
            }
        }

        let query_display = if self.command_query.is_empty() {
            "Filter commands by name, category, or shortcut...".to_string()
        } else {
            format!("{}|", self.command_query)
        };
        let query_is_empty = self.command_query.is_empty();
        let app_icon = cx.global::<AppRegistry>().app_icon.clone();

        div()
            .id("command-popup-backdrop")
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .bg(rgba(0x09090b88))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(52.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_command_popup(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("command-popup-card")
                    .key_context(COMMAND_POPUP_CONTEXT)
                    .track_focus(&self.command_focus_handle)
                    .on_key_down(cx.listener(Self::handle_command_popup_key_down))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(580.))
                    .max_h(px(520.))
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(rgb(0x18181b))
                    .text_color(rgb(0xf4f4f5))
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(img(app_icon).size(px(18.)))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child("Commands & Shortcuts"),
                                    )
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x27272a))
                                            .text_color(rgb(0xa1a1aa))
                                            .child(format!("{match_len}")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_color(rgb(0xa1a1aa))
                                            .child("↑↓ Navigate · ↵ Run · Esc Close"),
                                    )
                                    .child(
                                        div()
                                            .id("command-popup-close")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x27272a))
                                            .text_color(rgb(0xf4f4f5))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x3f3f46)))
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.close_command_popup(window, cx);
                                            }))
                                            .child("Close"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .px_3()
                            .py_2()
                            .bg(rgb(0x09090b))
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x1e3a8a))
                                            .text_color(rgb(0xe0f2fe))
                                            .font_weight(FontWeight::BOLD)
                                            .child("/"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_color(if query_is_empty {
                                                rgb(0x71717a)
                                            } else {
                                                rgb(0xffffff)
                                            })
                                            .child(query_display),
                                    ),
                            )
                            .when(!query_is_empty, |bar| {
                                bar.child(
                                    div()
                                        .id("command-popup-clear")
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x27272a))
                                        .text_color(rgb(0xd4d4d8))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0x3f3f46)))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.command_query.clear();
                                            this.selected_command_idx = 0;
                                            cx.notify();
                                        }))
                                        .child("Clear"),
                                )
                            }),
                    )
                    .child(
                        div()
                            .id("command-popup-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(list_items),
                    ),
            )
    }

    fn render_command_row(
        &self,
        idx: usize,
        entry: &CommandEntry,
        is_selected: bool,
        show_category_tag: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cmd_id = entry.id;
        div()
            .id(("cmd-row", idx))
            .px_2p5()
            .py_1p5()
            .rounded_sm()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .bg(if is_selected {
                rgb(0x1e3a8a)
            } else {
                rgb(0x18181b)
            })
            .border_1()
            .border_color(if is_selected {
                rgb(0x3b82f6)
            } else {
                rgb(0x27272a)
            })
            .hover(|s| {
                if is_selected {
                    s.bg(rgb(0x1d4ed8))
                } else {
                    s.bg(rgb(0x27272a))
                }
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, window, cx| {
                this.execute_command(cmd_id, window, cx);
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .child(entry.title)
                            .when(show_category_tag, |row| {
                                row.child(
                                    div()
                                        .px_1()
                                        .rounded_sm()
                                        .bg(rgb(0x27272a))
                                        .text_color(rgb(0x93c5fd))
                                        .child(entry.category),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if is_selected {
                                rgb(0xdbeafe)
                            } else {
                                rgb(0xa1a1aa)
                            })
                            .child(entry.description),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .bg(if is_selected {
                        rgb(0x1d4ed8)
                    } else {
                        rgb(0x27272a)
                    })
                    .border_1()
                    .border_color(if is_selected {
                        rgb(0x60a5fa)
                    } else {
                        rgb(0x3f3f46)
                    })
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(0xf4f4f5))
                    .child(entry.shortcut),
            )
            .into_any_element()
    }

    fn render_search_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let search = &self.session.read(cx).search;
        let query = search.query.clone();
        let regex_mode = search.regex_mode;
        let count_str = if search.matches.is_empty() {
            if query.is_empty() {
                "Type to search".to_string()
            } else {
                "0 / 0".to_string()
            }
        } else {
            format!("{} / {}", search.active_idx + 1, search.matches.len())
        };
        let indexing = self.session.read(cx).all_layouts.is_none();

        div()
            .id("search-bar")
            .key_context(SEARCH_BAR_CONTEXT)
            .track_focus(&self.search_focus_handle)
            .on_key_down(cx.listener(Self::handle_search_key_down))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .px_3()
            .py_1p5()
            .bg(rgb(0x27272a))
            .border_b_1()
            .border_color(rgb(0x3f3f46))
            .text_xs()
            .text_color(rgb(0xf4f4f5))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x93c5fd))
                            .child("Find"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .max_w(px(360.))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(rgb(0x09090b))
                            .border_1()
                            .border_color(rgb(0x3b82f6))
                            .text_color(if query.is_empty() {
                                rgb(0x71717a)
                            } else {
                                rgb(0xffffff)
                            })
                            .child(if query.is_empty() {
                                "Search text (or prefix re: for regex)...".to_string()
                            } else {
                                format!("{query}|")
                            }),
                    )
                    .child(toolbar_btn(
                        "search-regex",
                        if regex_mode { ".* Regex ON" } else { ".* Regex" },
                        regex_mode,
                        cx.listener(|this, _, _, cx| {
                            let (q, r) = {
                                let s = &this.session.read(cx).search;
                                (s.query.clone(), !s.regex_mode)
                            };
                            this.session.update(cx, |s, cx| s.update_search(q, r, cx));
                        }),
                    ))
                    .child(
                        div()
                            .text_color(rgb(0xd4d4d8))
                            .child(if indexing {
                                format!("{count_str} (indexing...)")
                            } else {
                                count_str
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(toolbar_btn(
                        "search-prev",
                        "Prev",
                        false,
                        cx.listener(|this, _, _, cx| this.step_search(-1, cx)),
                    ))
                    .child(toolbar_btn(
                        "search-next",
                        "Next",
                        false,
                        cx.listener(|this, _, _, cx| this.step_search(1, cx)),
                    ))
                    .child(toolbar_btn(
                        "search-close",
                        "Close",
                        false,
                        cx.listener(|this, _, window, cx| this.close_search_bar(window, cx)),
                    )),
            )
    }

    fn render_toc_popup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let toc_entries = &self.session.read(cx).toc_entries;
        let filtered = filter_toc(toc_entries, &self.toc_query);
        let len = filtered.len();
        let selected_idx = if len > 0 {
            Some(self.toc_selected_idx.min(len - 1))
        } else {
            None
        };
        let query_is_empty = self.toc_query.is_empty();
        let query_display = if query_is_empty {
            "Filter headings or p.<num>...".to_string()
        } else {
            format!("{}|", self.toc_query)
        };

        let mut rows: Vec<AnyElement> = Vec::with_capacity(len.max(1));
        if filtered.is_empty() {
            rows.push(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0xa1a1aa))
                    .child("No matching Table of Contents headings.")
                    .into_any_element(),
            );
        } else {
            for (idx, entry) in filtered.into_iter().enumerate() {
                let is_selected = selected_idx == Some(idx);
                let target_page = entry.page;
                let target_y = entry.top_norm_y;
                let indent_px = ((entry.level.saturating_sub(1)) as f32 * 14.0).min(42.0);
                let kind_badge = if entry.is_inferred { "Auto" } else { "TOC" };
                rows.push(
                    div()
                        .id(("toc-row", idx))
                        .px_2p5()
                        .py_1p5()
                        .rounded_sm()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .bg(if is_selected {
                            rgb(0x1e3a8a)
                        } else {
                            rgb(0x18181b)
                        })
                        .border_1()
                        .border_color(if is_selected {
                            rgb(0x3b82f6)
                        } else {
                            rgb(0x27272a)
                        })
                        .hover(|s| {
                            if is_selected {
                                s.bg(rgb(0x1d4ed8))
                            } else {
                                s.bg(rgb(0x27272a))
                            }
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.jump_to_toc_entry(target_page, target_y, window, cx);
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .pl(px(indent_px))
                                .text_xs()
                                .font_weight(if entry.level <= 1 {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(rgb(0xffffff))
                                .child(entry.title.clone()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .px_1()
                                        .rounded_sm()
                                        .bg(rgb(0x27272a))
                                        .text_color(rgb(0x93c5fd))
                                        .text_xs()
                                        .child(kind_badge),
                                )
                                .child(
                                    div()
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(0x27272a))
                                        .text_color(rgb(0xd4d4d8))
                                        .text_xs()
                                        .child(format!("p.{}", entry.page.one_based())),
                                ),
                        )
                        .into_any_element(),
                );
            }
        }

        div()
            .id("toc-popup-backdrop")
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .bg(rgba(0x09090b88))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(52.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_toc_popup(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("toc-popup-card")
                    .key_context(OVERLAY_POPUP_CONTEXT)
                    .track_focus(&self.toc_focus_handle)
                    .on_key_down(cx.listener(Self::handle_toc_key_down))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(560.))
                    .max_h(px(520.))
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(rgb(0x18181b))
                    .text_color(rgb(0xf4f4f5))
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child("Table of Contents"),
                                    )
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x27272a))
                                            .text_color(rgb(0xa1a1aa))
                                            .child(format!("{len}")),
                                    ),
                            )
                            .child(toolbar_btn(
                                "toc-close",
                                "Close",
                                false,
                                cx.listener(|this, _, window, cx| this.close_toc_popup(window, cx)),
                            )),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .bg(rgb(0x09090b))
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .text_color(if query_is_empty {
                                rgb(0x71717a)
                            } else {
                                rgb(0xffffff)
                            })
                            .child(query_display),
                    )
                    .child(
                        div()
                            .id("toc-popup-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(rows),
                    ),
            )
    }

    fn render_library_modal(
        &self,
        kind: LibraryModalKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let items = self.collect_library_items(kind, cx);
        let len = items.len();
        let selected_idx = if len > 0 {
            Some(self.library_selected_idx.min(len - 1))
        } else {
            None
        };
        let (title, placeholder, empty_msg) = match kind {
            LibraryModalKind::RecentDocs => (
                "Recent PDFs (Library)",
                "Filter recent documents by title or path...",
                "No recent PDFs indexed yet. Open a PDF file to add it to your library.",
            ),
            LibraryModalKind::GlobalHighlights => (
                "Global Highlights & Notes",
                "Search highlights & notes across all opened PDFs...",
                "No matching highlights or notes across indexed documents.",
            ),
        };
        let query_is_empty = self.library_query.is_empty();
        let query_display = if query_is_empty {
            placeholder.to_string()
        } else {
            format!("{}|", self.library_query)
        };

        let mut rows: Vec<AnyElement> = Vec::with_capacity(len.max(1));
        if items.is_empty() {
            rows.push(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(rgb(0xa1a1aa))
                    .child(empty_msg)
                    .into_any_element(),
            );
        } else {
            for (idx, item) in items.into_iter().enumerate() {
                let is_selected = selected_idx == Some(idx);
                let target_path = item.target_path;
                let target_page = item.target_page;
                rows.push(
                    div()
                        .id(("lib-row", idx))
                        .px_2p5()
                        .py_1p5()
                        .rounded_sm()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .bg(if is_selected {
                            rgb(0x1e3a8a)
                        } else {
                            rgb(0x18181b)
                        })
                        .border_1()
                        .border_color(if is_selected {
                            rgb(0x3b82f6)
                        } else {
                            rgb(0x27272a)
                        })
                        .hover(|s| {
                            if is_selected {
                                s.bg(rgb(0x1d4ed8))
                            } else {
                                s.bg(rgb(0x27272a))
                            }
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.activate_library_item(
                                target_path.clone(),
                                target_page,
                                window,
                                cx,
                            );
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xffffff))
                                        .child(item.title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if is_selected {
                                            rgb(0xdbeafe)
                                        } else {
                                            rgb(0xa1a1aa)
                                        })
                                        .child(item.subtitle),
                                ),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(rgb(0x27272a))
                                .text_color(rgb(0xd4d4d8))
                                .text_xs()
                                .child(item.badge),
                        )
                        .into_any_element(),
                );
            }
        }

        div()
            .id("library-popup-backdrop")
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .bg(rgba(0x09090b88))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(52.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_library_modal(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("library-popup-card")
                    .key_context(OVERLAY_POPUP_CONTEXT)
                    .track_focus(&self.library_focus_handle)
                    .on_key_down(cx.listener(Self::handle_library_key_down))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(620.))
                    .max_h(px(520.))
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(rgb(0x18181b))
                    .text_color(rgb(0xf4f4f5))
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x27272a))
                                            .text_color(rgb(0xa1a1aa))
                                            .child(format!("{len}")),
                                    ),
                            )
                            .child(toolbar_btn(
                                "library-close",
                                "Close",
                                false,
                                cx.listener(|this, _, window, cx| {
                                    this.close_library_modal(window, cx)
                                }),
                            )),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .bg(rgb(0x09090b))
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .text_color(if query_is_empty {
                                rgb(0x71717a)
                            } else {
                                rgb(0xffffff)
                            })
                            .child(query_display),
                    )
                    .child(
                        div()
                            .id("library-popup-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_2()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .children(rows),
                    ),
            )
    }
}

impl Render for WindowWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_close_window {
            self.pending_close_window = false;
            window.remove_window();
            return div().into_any_element();
        }
        if let Some(fh) = self.pending_focus.take() {
            window.focus(&fh, cx);
        }

        let desired_title = format!("Pinleaf — {}", self.session.read(cx).title);
        if self.last_window_title != desired_title {
            self.last_window_title = desired_title.clone();
            window.set_window_title(&desired_title);
        }

        let tree_view = self.render_split_node(&self.root, cx);

        div()
            .id("workspace-root")
            .key_context(APP_CONTEXT)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xe4e4e7))
            .on_action(cx.listener(|this, _: &OpenFile, _, cx| this.prompt_open_file(cx)))
            .on_action(cx.listener(|this, _: &OpenDemo, _, cx| this.open_demo_session(cx)))
            .on_action(cx.listener(|this, _: &SavePdf, _, cx| this.trigger_save(false, cx)))
            .on_action(cx.listener(|this, _: &SavePdfAs, _, cx| this.trigger_save(true, cx)))
            .on_action(cx.listener(|this, _: &UndoAnnot, _, cx| {
                this.session.update(cx, |s, cx| s.undo(cx));
            }))
            .on_action(cx.listener(|this, _: &RedoAnnot, _, cx| {
                this.session.update(cx, |s, cx| s.redo(cx));
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar_open = !this.sidebar_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleCommandPopup, window, cx| {
                this.toggle_command_popup(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSearch, window, cx| {
                this.open_search_bar(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NextSearchMatch, _, cx| {
                this.step_search(1, cx);
            }))
            .on_action(cx.listener(|this, _: &PrevSearchMatch, _, cx| {
                this.step_search(-1, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTocPopup, window, cx| {
                this.toggle_toc_popup(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenRecentPopup, window, cx| {
                this.toggle_library_modal(LibraryModalKind::RecentDocs, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenGlobalHighlightsPopup, window, cx| {
                this.toggle_library_modal(LibraryModalKind::GlobalHighlights, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleAutoReload, _, cx| {
                this.session.update(cx, |s, cx| s.toggle_auto_reload(cx));
            }))
            .on_action(cx.listener(|this, _: &CycleColorMode, _, cx| {
                this.session.update(cx, |s, cx| s.cycle_color_mode(cx));
            }))
            .on_drop(cx.listener(|this, dropped: &gpui::ExternalPaths, window, cx| {
                let files: Vec<PathBuf> = dropped
                    .paths()
                    .iter()
                    .filter(|p| p.is_file())
                    .cloned()
                    .collect();
                if files.is_empty() {
                    return;
                }
                let pdfs: Vec<PathBuf> = files
                    .iter()
                    .filter(|p| {
                        p.extension()
                            .and_then(|e| e.to_str())
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
                    })
                    .cloned()
                    .collect();
                let mut iter = if pdfs.is_empty() { files } else { pdfs }.into_iter();
                if let Some(first) = iter.next() {
                    this.open_external_path(first, cx);
                    window.activate();
                }
                for extra in iter {
                    crate::open_pdf_in_new_window(extra, cx);
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if let Some((split_id, axis, ref bounds_cell)) = this.active_split_drag {
                    let b = bounds_cell.get();
                    let ratio = match axis {
                        Axis::Horizontal => {
                            (event.position.x.as_f32() - b.origin.x.as_f32())
                                / b.size.width.as_f32().max(1.0)
                        }
                        Axis::Vertical => {
                            (event.position.y.as_f32() - b.origin.y.as_f32())
                                / b.size.height.as_f32().max(1.0)
                        }
                    };
                    if this.root.update_split_ratio(split_id, ratio) {
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.active_split_drag = None;
                }),
            )
            .child(self.render_toolbar(cx))
            .when(self.search_bar_open, |root| {
                root.child(self.render_search_bar(cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(div().flex_1().min_w_0().min_h_0().child(tree_view))
                    .when(self.sidebar_open, |row| row.child(self.render_sidebar(cx))),
            )
            .when(self.toc_popup_open, |root| root.child(self.render_toc_popup(cx)))
            .when_some(self.library_modal, |root, kind| {
                root.child(self.render_library_modal(kind, cx))
            })
            .when(self.command_popup_open, |root| {
                root.child(self.render_command_popup(cx))
            })
            .into_any_element()
    }
}

pub fn open_workspace_window(
    session: Entity<DocumentSession>,
    parent_viewport: Option<ViewportId>,
    anchor: ReadingAnchor,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(960.), px(820.)), cx);
    let title = format!("Pinleaf — {}", session.read(cx).title);
    let _ = cx.open_window(
        WindowOptions::default()
            .window_bounds(WindowBounds::Windowed(bounds))
            .titlebar(Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            })),
        move |window, cx| {
            cx.new(|cx| WindowWorkspace::new_with_anchor(session, parent_viewport, anchor, window, cx))
        },
    );
    cx.activate(true);
}

fn toolbar_btn(
    id: &'static str,
    label: impl Into<gpui::SharedString>,
    highlighted: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_2p5()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .bg(if highlighted {
            rgb(0x2563eb)
        } else {
            rgb(0x27272a)
        })
        .text_color(rgb(0xffffff))
        .hover(|s| s.bg(if highlighted { rgb(0x1d4ed8) } else { rgb(0x3f3f46) }))
        .child(label.into())
        .on_click(on_click)
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars).collect();
        out.push_str("...");
        out
    }
}

fn apply_text_edit_keystroke(buf: &mut String, event: &KeyDownEvent, cx: &mut App) -> bool {
    let ks = &event.keystroke;
    if ks.key == "backspace" {
        return buf.pop().is_some();
    }
    if (ks.modifiers.platform || ks.modifiers.control) && ks.key == "v" {
        if let Some(clip) = cx.read_from_clipboard()
            && let Some(text) = clip.text()
        {
            let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
            if !filtered.is_empty() {
                buf.push_str(&filtered);
                return true;
            }
        }
        return false;
    }
    if !ks.modifiers.platform
        && !ks.modifiers.control
        && let Some(ch) = &ks.key_char
    {
        let printable: String = ch.chars().filter(|c| !c.is_control()).collect();
        if !printable.is_empty() {
            buf.push_str(&printable);
            return true;
        }
    }
    false
}
