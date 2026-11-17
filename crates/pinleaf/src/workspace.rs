use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, AnyWindowHandle, App, Axis, Bounds, ClipboardItem, Context, Entity, FocusHandle,
    FontWeight, KeyDownEvent, Keystroke, MouseButton, MouseMoveEvent, PathPromptOptions, Pixels,
    Render, RenderImage, ScrollStrategy, Subscription, TitlebarOptions, UniformListScrollHandle,
    Window, WindowBounds, WindowOptions, canvas, div, img, prelude::*, px, rgb, rgba, size,
    uniform_list,
};
use pinleaf_core::{
    AnnotationId, AnnotationKind, FreezeState, PageIndex, ReadingAnchor, ViewportId, ZoomSpec,
    find_active_portal, highlight_category_spec,
};
use pinleaf_engine::filter_toc;

use crate::actions::{
    APP_CONTEXT, COMMAND_POPUP_CONTEXT, CycleColorMode, GoToPage, NextSearchMatch,
    OVERLAY_POPUP_CONTEXT, OpenDemo, OpenExtensionsPopup, OpenFile, OpenGlobalHighlightsPopup,
    OpenRecentPopup, OpenSearch, PrevSearchMatch, RedoAnnot, SEARCH_BAR_CONTEXT, SavePdf,
    SavePdfAs, ShowAbout, ToggleAutoReload, ToggleCommandPopup, TogglePresentationMode,
    ToggleSidebar, ToggleSmartDarkMode, ToggleThumbnailGrid, ToggleThumbnails, ToggleTocPopup,
    UndoAnnot,
};
use crate::app_state::{AppRegistry, navigate_viewport_globally, sync_portal_followers};
use crate::command_popup::{CommandEntry, CommandId, filter_commands, parse_page_jump_query};
use crate::external::run_shell_extension_blocking;
use crate::library::{GlobalHighlightHit, IndexedHighlight, matches_highlight};
use crate::session::{ColorMode, DocumentSession};
use crate::viewport::{HintModeKind, PdfViewport, ViewportEvent};

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
    Extensions,
}

#[derive(Clone)]
struct LibraryRowItem {
    title: String,
    subtitle: String,
    badge: String,
    target_path: Option<PathBuf>,
    target_page: PageIndex,
    extension_cmd: Option<String>,
}

#[derive(Clone)]
struct ThumbPageMeta {
    page: PageIndex,
    norm_w: f32,
    norm_h: f32,
    is_active: bool,
    frozen_trail_num: Option<usize>,
    annot_count: usize,
    portal_count: usize,
}

pub struct WindowWorkspace {
    session: Entity<DocumentSession>,
    root: SplitNode,
    focused_viewport: ViewportId,
    sidebar_open: bool,
    sidebar_category_filter: Option<char>,
    thumbnails_open: bool,
    thumbs_scroll_handle: UniformListScrollHandle,
    thumbnail_grid_open: bool,
    thumbnail_grid_selected: usize,
    thumbnail_grid_scroll_handle: UniformListScrollHandle,
    thumbnail_grid_focus_handle: FocusHandle,
    goto_page_open: bool,
    goto_page_query: String,
    goto_page_focus_handle: FocusHandle,
    brand_menu_open: bool,
    about_popup_open: bool,
    about_focus_handle: FocusHandle,
    presentation_mode: bool,
    pre_presentation_zoom: Option<ZoomSpec>,
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
    last_seen_status: Option<(String, bool)>,
    status_toast_epoch: u64,
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

        let _session_obs = Self::observe_session(&session, cx);
        session.update(cx, |s, cx| s.trigger_background_text_index(cx));

        let mut ws = Self {
            session,
            root: SplitNode::Leaf { viewport },
            focused_viewport: vp_id,
            sidebar_open: false,
            sidebar_category_filter: None,
            thumbnails_open: false,
            thumbs_scroll_handle: UniformListScrollHandle::new(),
            thumbnail_grid_open: false,
            thumbnail_grid_selected: 0,
            thumbnail_grid_scroll_handle: UniformListScrollHandle::new(),
            thumbnail_grid_focus_handle: cx.focus_handle(),
            goto_page_open: false,
            goto_page_query: String::new(),
            goto_page_focus_handle: cx.focus_handle(),
            brand_menu_open: false,
            about_popup_open: false,
            about_focus_handle: cx.focus_handle(),
            presentation_mode: false,
            pre_presentation_zoom: None,
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
            last_seen_status: None,
            status_toast_epoch: 0,
        };
        ws.sync_status_toast_timer(cx);
        ws
    }

    fn observe_session(
        session: &Entity<DocumentSession>,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.observe(session, |this, _, cx| {
            this.sync_status_toast_timer(cx);
            cx.notify();
        })
    }

    fn sync_status_toast_timer(&mut self, cx: &mut Context<Self>) {
        let current = self.session.read(cx).status_message.clone();
        if current == self.last_seen_status {
            return;
        }
        self.last_seen_status = current.clone();
        self.status_toast_epoch = self.status_toast_epoch.wrapping_add(1);
        if let Some((_, is_err)) = current {
            let epoch = self.status_toast_epoch;
            let delay_ms = if is_err { 4500 } else { 3000 };
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(delay_ms))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.status_toast_epoch == epoch {
                        this.last_seen_status = None;
                        this.session.update(cx, |s, cx| {
                            s.status_message = None;
                            cx.notify();
                        });
                    }
                });
            })
            .detach();
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
            ViewportEvent::ReadingPositionChanged {
                id,
                page,
                focal_y,
            } => {
                if ws.thumbnails_open && id == ws.focused_viewport {
                    ws.thumbs_scroll_handle
                        .scroll_to_item(page.as_usize(), ScrollStrategy::Nearest);
                }
                let sess = ws.session.clone();
                cx.spawn(async move |_, cx| {
                    cx.update(|cx| {
                        sync_portal_followers(&sess, id, page, focal_y, cx);
                    });
                })
                .detach();
            }
            ViewportEvent::RequestEnsurePortalFollower { source_id, dst } => {
                ws.ensure_portal_follower_for(source_id, dst, cx);
            }
            ViewportEvent::RequestTogglePortalFollower(id) => {
                ws.toggle_portal_follower(id, cx);
            }
            ViewportEvent::RequestGoToPage(id) => {
                ws.focused_viewport = id;
                ws.open_goto_page_popup_deferred(cx);
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

    fn toggle_portal_follower(&mut self, target_id: ViewportId, cx: &mut Context<Self>) {
        let Some(target_vp) = self.root.find_viewport(target_id, cx) else {
            return;
        };
        let was_follower = target_vp.read(cx).portal_follower;
        if !was_follower && self.root.leaf_count() == 1 {
            let src_anchor = target_vp.read(cx).anchor;
            let dst_anchor = {
                let s = self.session.read(cx);
                find_active_portal(&s.portals, src_anchor.page, src_anchor.focal_norm.y)
                    .map(|p| p.dst)
                    .unwrap_or(src_anchor)
            };
            self.spawn_portal_split(target_id, dst_anchor, cx);
            return;
        }
        target_vp.update(cx, |vp, cx| {
            vp.portal_follower = !was_follower;
            if !vp.portal_follower {
                vp.active_portal_label = None;
            }
            cx.notify();
        });
        let now_follower = !was_follower;
        self.session.update(cx, |s, cx| {
            s.status_message = Some((
                format!(
                    "Portal follower: {}",
                    if now_follower { "ON" } else { "OFF" }
                ),
                false,
            ));
            cx.notify();
        });
    }

    fn ensure_portal_follower_for(
        &mut self,
        source_id: ViewportId,
        dst: ReadingAnchor,
        cx: &mut Context<Self>,
    ) {
        let has_follower = cx.global::<AppRegistry>().viewports.iter().any(|(&id, entry)| {
            if id == source_id {
                return false;
            }
            entry
                .viewport
                .upgrade()
                .is_some_and(|vp| {
                    let v = vp.read(cx);
                    v.session == self.session && v.portal_follower
                })
        });
        if has_follower {
            let sess = self.session.clone();
            let (cur_page, cur_y) = {
                let vp = self.active_viewport_entity(cx);
                let a = vp.read(cx).anchor;
                (a.page, a.focal_norm.y)
            };
            cx.spawn(async move |_, cx| {
                cx.update(|cx| {
                    sync_portal_followers(&sess, source_id, cur_page, cur_y, cx);
                });
            })
            .detach();
            return;
        }
        self.spawn_portal_split(source_id, dst, cx);
    }

    fn spawn_portal_split(
        &mut self,
        source_id: ViewportId,
        dst: ReadingAnchor,
        cx: &mut Context<Self>,
    ) {
        let Some(window_handle) = cx
            .global::<AppRegistry>()
            .viewports
            .get(&source_id)
            .map(|e| e.window)
        else {
            return;
        };
        let child = self.spawn_child_viewport(source_id, dst, window_handle, cx);
        let portal_label = {
            let s = self.session.read(cx);
            s.portals.last().map(|p| p.label.clone())
        };
        child.update(cx, |vp, _| {
            vp.portal_follower = true;
            vp.active_portal_label = portal_label;
            vp.hide_header = self.presentation_mode;
        });

        let split_id = self.next_split_id;
        self.next_split_id += 1;
        self.root
            .replace_leaf_with_split(source_id, split_id, Axis::Horizontal, child, cx);
        self.sync_current_session_library(cx);
        cx.notify();
    }

    pub fn toggle_presentation_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.presentation_mode = !self.presentation_mode;
        if window.is_fullscreen() != self.presentation_mode {
            window.toggle_fullscreen();
        }
        let mut vps = Vec::new();
        self.root.all_viewports(&mut vps);
        for vp in &vps {
            vp.update(cx, |vp, cx| {
                vp.hide_header = self.presentation_mode;
                cx.notify();
            });
        }
        let active = self.active_viewport_entity(cx);
        if self.presentation_mode {
            self.sidebar_open = false;
            self.pre_presentation_zoom = Some(active.read(cx).anchor.zoom);
            active.update(cx, |vp, cx| {
                vp.anchor.zoom = ZoomSpec::FitPage;
                vp.pan_x = 0.0;
                cx.notify();
            });
        } else if let Some(prev) = self.pre_presentation_zoom.take() {
            active.update(cx, |vp, cx| {
                vp.anchor.zoom = prev;
                cx.notify();
            });
        }
        cx.notify();
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
        self.toc_popup_open = false;
        self.library_modal = None;
        self.goto_page_open = false;
        self.thumbnail_grid_open = false;
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
        self.restore_viewport_focus(window, cx);
        cx.notify();
    }

    fn toggle_command_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_popup_open {
            self.close_command_popup(window, cx);
        } else {
            self.open_command_popup(window, cx);
        }
    }

    fn restore_viewport_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        let fh = self.active_viewport_entity(cx).read(cx).focus_handle().clone();
        window.focus(&fh, cx);
    }

    fn jump_to_page_from_ui(&mut self, page: PageIndex, split_new: bool, cx: &mut Context<Self>) {
        let focused_id = self.focused_viewport;
        if split_new {
            if let Some(child) = self.freeze_and_split(focused_id, Axis::Horizontal, cx) {
                child.update(cx, |vp, cx| {
                    vp.jump_to_page_offset_recorded(page, 0.0, cx);
                });
            }
        } else {
            self.navigate_focused_or_smart_route(page, 0.0, cx);
        }
        if self.thumbnails_open {
            self.thumbs_scroll_handle
                .scroll_to_item(page.as_usize(), ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    fn toggle_thumbnails(&mut self, cx: &mut Context<Self>) {
        self.thumbnails_open = !self.thumbnails_open;
        if self.thumbnails_open {
            let active_pg = self.active_viewport_entity(cx).read(cx).anchor.page.as_usize();
            self.thumbs_scroll_handle
                .scroll_to_item(active_pg, ScrollStrategy::Center);
        }
        cx.notify();
    }

    fn open_thumbnail_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_popup_open = false;
        self.toc_popup_open = false;
        self.library_modal = None;
        self.goto_page_open = false;
        self.thumbnail_grid_open = true;
        let active_pg = self.active_viewport_entity(cx).read(cx).anchor.page.as_usize();
        let count = self.session.read(cx).page_count();
        self.thumbnail_grid_selected = if count == 0 {
            0
        } else {
            active_pg.min(count - 1)
        };
        self.thumbnail_grid_scroll_handle
            .scroll_to_item(self.thumbnail_grid_selected / 4, ScrollStrategy::Center);
        window.focus(&self.thumbnail_grid_focus_handle, cx);
        cx.notify();
    }

    fn close_thumbnail_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.thumbnail_grid_open = false;
        self.restore_viewport_focus(window, cx);
        cx.notify();
    }

    fn toggle_thumbnail_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.thumbnail_grid_open {
            self.close_thumbnail_grid(window, cx);
        } else {
            self.open_thumbnail_grid(window, cx);
        }
    }

    fn open_goto_page_popup_deferred(&mut self, cx: &mut Context<Self>) {
        self.command_popup_open = false;
        self.toc_popup_open = false;
        self.library_modal = None;
        self.thumbnail_grid_open = false;
        self.goto_page_open = true;
        self.goto_page_query.clear();
        self.pending_focus = Some(self.goto_page_focus_handle.clone());
        cx.notify();
    }

    fn open_goto_page_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_goto_page_popup_deferred(cx);
        self.pending_focus = None;
        window.focus(&self.goto_page_focus_handle, cx);
    }

    fn close_goto_page_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.goto_page_open = false;
        self.goto_page_query.clear();
        self.restore_viewport_focus(window, cx);
        cx.notify();
    }

    pub(crate) fn open_about_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.brand_menu_open = false;
        self.command_popup_open = false;
        self.toc_popup_open = false;
        self.goto_page_open = false;
        self.thumbnail_grid_open = false;
        self.library_modal = None;
        self.about_popup_open = true;
        window.focus(&self.about_focus_handle, cx);
        cx.notify();
    }

    fn close_about_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.about_popup_open = false;
        self.restore_viewport_focus(window, cx);
        cx.notify();
    }

    fn toggle_brand_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.brand_menu_open = !self.brand_menu_open;
        if self.brand_menu_open {
            self.about_popup_open = false;
            self.command_popup_open = false;
            self.toc_popup_open = false;
            self.goto_page_open = false;
            self.thumbnail_grid_open = false;
            self.library_modal = None;
        } else {
            self.restore_viewport_focus(window, cx);
        }
        cx.notify();
    }

    fn close_brand_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.brand_menu_open {
            self.brand_menu_open = false;
            self.restore_viewport_focus(window, cx);
            cx.notify();
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
            CommandId::ScrollScreenDown => vp.update(cx, |vp, cx| vp.scroll_screen_down(cx)),
            CommandId::ScrollScreenUp => vp.update(cx, |vp, cx| vp.scroll_screen_up(cx)),
            CommandId::JumpBack => vp.update(cx, |vp, cx| vp.go_back(cx)),
            CommandId::JumpForward => vp.update(cx, |vp, cx| vp.go_forward(cx)),
            CommandId::FirstPage => vp.update(cx, |vp, cx| vp.go_first_page(cx)),
            CommandId::LastPage => vp.update(cx, |vp, cx| vp.go_last_page(cx)),
            CommandId::OpenSearch => self.open_search_bar(window, cx),
            CommandId::NextSearchMatch => self.step_search(1, cx),
            CommandId::PrevSearchMatch => self.step_search(-1, cx),
            CommandId::ToggleTocPopup => self.toggle_toc_popup(window, cx),
            CommandId::GoToPage => self.open_goto_page_popup(window, cx),
            CommandId::ToggleThumbnails => self.toggle_thumbnails(cx),
            CommandId::ToggleThumbnailGrid => self.toggle_thumbnail_grid(window, cx),
            CommandId::BeginSetMark => vp.update(cx, |vp, cx| vp.begin_set_mark(cx)),
            CommandId::BeginJumpMark => vp.update(cx, |vp, cx| vp.begin_jump_mark(cx)),
            CommandId::ZoomIn => vp.update(cx, |vp, cx| vp.zoom_in(cx)),
            CommandId::ZoomOut => vp.update(cx, |vp, cx| vp.zoom_out(cx)),
            CommandId::ZoomFitWidth => vp.update(cx, |vp, cx| vp.zoom_fit_width(cx)),
            CommandId::ZoomFitContentWidth => {
                vp.update(cx, |vp, cx| vp.zoom_fit_content_width(cx));
            }
            CommandId::CycleColorMode => self.session.update(cx, |s, cx| s.cycle_color_mode(cx)),
            CommandId::ToggleSmartDarkMode => {
                self.session.update(cx, |s, cx| s.toggle_smart_dark_mode(cx));
            }
            CommandId::ToggleReadingRuler => vp.update(cx, |vp, cx| vp.toggle_reading_ruler(cx)),
            CommandId::ToggleAutoScroll => vp.update(cx, |vp, cx| vp.toggle_auto_scroll(cx)),
            CommandId::TogglePresentationMode => self.toggle_presentation_mode(window, cx),
            CommandId::LinkPortal => vp.update(cx, |vp, cx| vp.link_portal(cx)),
            CommandId::TogglePortalFollower => self.toggle_portal_follower(vp_id, cx),
            CommandId::DeleteNearestPortal => vp.update(cx, |vp, cx| vp.delete_nearest_portal(cx)),
            CommandId::BeginHintJump => {
                vp.update(cx, |vp, cx| vp.begin_hint_mode(HintModeKind::Jump, cx));
            }
            CommandId::BeginHintOverview => {
                vp.update(cx, |vp, cx| vp.begin_hint_mode(HintModeKind::Overview, cx));
            }
            CommandId::BeginHintSelect => {
                vp.update(cx, |vp, cx| vp.begin_hint_mode(HintModeKind::SelectStart, cx));
            }
            CommandId::BeginTypedHighlight => {
                vp.update(cx, |vp, cx| vp.begin_typed_highlight(cx));
            }
            CommandId::SynctexInverseJump => {
                vp.update(cx, |vp, cx| vp.synctex_inverse_here(cx));
            }
            CommandId::OpenExtensionsPopup => {
                self.open_library_modal(LibraryModalKind::Extensions, window, cx);
            }
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
            CommandId::ShowAbout => self.open_about_popup(window, cx),
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

        let page_count = self.session.read(cx).page_count();
        let page_jump = parse_page_jump_query(&self.command_query, page_count);
        let matches = filter_commands(&self.command_query);
        let match_len = matches.len();
        let total_rows = match_len + usize::from(page_jump.is_some());

        if ks.key == "down"
            || (ks.modifiers.control && ks.key == "n")
            || (ks.key == "tab" && !ks.modifiers.shift)
        {
            if total_rows > 0 {
                let current = self.selected_command_idx.min(total_rows - 1);
                self.selected_command_idx = (current + 1) % total_rows;
                cx.notify();
            }
            return;
        }

        if ks.key == "up"
            || (ks.modifiers.control && ks.key == "p")
            || (ks.key == "tab" && ks.modifiers.shift)
        {
            if total_rows > 0 {
                let current = self.selected_command_idx.min(total_rows - 1);
                self.selected_command_idx = (current + total_rows - 1) % total_rows;
                cx.notify();
            }
            return;
        }

        if ks.key == "enter" {
            if let Some(target_page_1based) = page_jump {
                let idx = self.selected_command_idx.min(total_rows.saturating_sub(1));
                if idx == 0 || match_len == 0 {
                    self.close_command_popup(window, cx);
                    self.jump_to_page_from_ui(PageIndex(target_page_1based - 1), false, cx);
                    return;
                }
                let cmd_id = matches[idx - 1].id;
                self.execute_command(cmd_id, window, cx);
                return;
            }
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
                    extension_cmd: None,
                })
                .collect(),
            LibraryModalKind::Extensions => {
                let query = self.library_query.trim().to_ascii_lowercase();
                let mut rows: Vec<LibraryRowItem> = reg
                    .user_config
                    .extensions
                    .iter()
                    .map(|ext| LibraryRowItem {
                        title: ext.name.clone(),
                        subtitle: ext.command.clone(),
                        badge: ext.key.clone().unwrap_or_else(|| "User".to_string()),
                        target_path: None,
                        target_page: PageIndex(0),
                        extension_cmd: Some(ext.command.clone()),
                    })
                    .collect();

                let builtins = [
                    (
                        "Copy Markdown Citation",
                        "__pinleaf_copy_cite__",
                        "Built-in",
                    ),
                    (
                        "Copy Quote to Clipboard",
                        "printf '%s' {selected_text} | pbcopy",
                        "Built-in",
                    ),
                    (
                        "Speak Selection / Line (macOS say)",
                        "say {selected_text}",
                        "macOS",
                    ),
                    (
                        "Reveal PDF in Finder",
                        "open -R {file_path}",
                        "macOS",
                    ),
                    (
                        "SyncTeX Jump to TeX Source",
                        "__pinleaf_synctex__",
                        "⇧⌘J",
                    ),
                ];
                for (name, cmd, badge) in builtins {
                    rows.push(LibraryRowItem {
                        title: name.to_string(),
                        subtitle: cmd.to_string(),
                        badge: badge.to_string(),
                        target_path: None,
                        target_page: PageIndex(0),
                        extension_cmd: Some(cmd.to_string()),
                    });
                }

                if !query.is_empty() {
                    rows.retain(|r| {
                        r.title.to_ascii_lowercase().contains(&query)
                            || r.subtitle.to_ascii_lowercase().contains(&query)
                    });
                }
                rows
            }
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
                            extension_cmd: None,
                        }
                    })
                    .collect()
            }
        }
    }

    fn run_extension_command(
        &mut self,
        cmd_template: String,
        cx: &mut Context<Self>,
    ) {
        if cmd_template == "__pinleaf_synctex__" {
            self.active_viewport_entity(cx)
                .update(cx, |vp, cx| vp.synctex_inverse_here(cx));
            return;
        }
        let ext_ctx = self
            .active_viewport_entity(cx)
            .update(cx, |vp, cx| vp.current_extension_context(cx));
        if cmd_template == "__pinleaf_copy_cite__" {
            let doc_title = self.session.read(cx).title.clone();
            let cite = if ext_ctx.selected_text.is_empty() {
                format!("{doc_title} (p.{})", ext_ctx.page_one_based)
            } else {
                format!(
                    "\"{}\" — {doc_title} (p.{})",
                    ext_ctx.selected_text, ext_ctx.page_one_based
                )
            };
            cx.write_to_clipboard(ClipboardItem::new_string(cite));
            self.session.update(cx, |s, cx| {
                s.status_message = Some(("Copied citation to clipboard".to_string(), false));
                cx.notify();
            });
            return;
        }

        let session = self.session.clone();
        cx.spawn(async move |_, cx| {
            let outcome = cx
                .background_spawn(async move {
                    run_shell_extension_blocking(&cmd_template, &ext_ctx)
                })
                .await;
            let _ = session.update(cx, |s, cx| {
                s.status_message = Some(match outcome {
                    Ok(msg) => (msg, false),
                    Err(err) => (err, true),
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn activate_library_item(
        &mut self,
        item: &LibraryRowItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_library_modal(window, cx);
        if let Some(ref ext_cmd) = item.extension_cmd {
            self.run_extension_command(ext_cmd.clone(), cx);
            return;
        }
        if let Some(ref path) = item.target_path {
            self.open_or_switch_path_at_page(path.clone(), item.target_page, cx);
        } else {
            self.navigate_focused_or_smart_route(item.target_page, 0.0, cx);
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
                self.activate_library_item(item, window, cx);
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
        self._session_obs = Self::observe_session(&new_session, cx);
        self.session = new_session;
        self.last_seen_status = None;
        self.sync_status_toast_timer(cx);
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
        let color_mode = session.color_mode;
        let app_icon = cx.global::<AppRegistry>().app_icon.clone();

        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_1p5()
            .px_2p5()
            .py_1()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xf4f4f5))
            .text_xs()
            .border_b_1()
            .border_color(rgb(0x27272a))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .id("pinleaf-brand-btn")
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .cursor_pointer()
                            .bg(if self.brand_menu_open {
                                rgb(0x2563eb)
                            } else {
                                rgb(0x1e3a8a)
                            })
                            .hover(|s| s.bg(rgb(0x1d4ed8)))
                            .text_color(rgb(0xe0f2fe))
                            .font_weight(FontWeight::BOLD)
                            .child(img(app_icon).size(px(16.)))
                            .child("Pinleaf")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_brand_menu(window, cx);
                            })),
                    )
                    .child(toolbar_btn(
                        "open-pdf",
                        "Open",
                        false,
                        cx.listener(|this, _, _, cx| this.prompt_open_file(cx)),
                    ))
                    .child(toolbar_btn(
                        "recent-btn",
                        "Recent",
                        self.library_modal == Some(LibraryModalKind::RecentDocs),
                        cx.listener(|this, _, window, cx| {
                            this.toggle_library_modal(LibraryModalKind::RecentDocs, window, cx);
                        }),
                    ))
                    .when(can_undo, |row| {
                        row.child(toolbar_btn(
                            "undo-btn",
                            "Undo",
                            false,
                            cx.listener(|this, _, _, cx| {
                                this.session.update(cx, |s, cx| s.undo(cx));
                            }),
                        ))
                    })
                    .when(can_redo, |row| {
                        row.child(toolbar_btn(
                            "redo-btn",
                            "Redo",
                            false,
                            cx.listener(|this, _, _, cx| {
                                this.session.update(cx, |s, cx| s.redo(cx));
                            }),
                        ))
                    })
                    .child(
                        div()
                            .id("save-btn")
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .cursor_pointer()
                            .bg(if dirty {
                                rgb(0xd97706)
                            } else {
                                rgb(0x27272a)
                            })
                            .text_color(rgb(0xffffff))
                            .when(dirty, |btn| btn.font_weight(FontWeight::SEMIBOLD))
                            .hover(|s| {
                                s.bg(if dirty {
                                    rgb(0xb45309)
                                } else {
                                    rgb(0x3f3f46)
                                })
                            })
                            .child(if dirty { "Save *" } else { "Save" })
                            .on_click(cx.listener(|this, _, _, cx| this.trigger_save(false, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1p5()
                    .child(toolbar_btn(
                        "thumbs-btn",
                        "Pages",
                        self.thumbnails_open,
                        cx.listener(|this, _, _, cx| this.toggle_thumbnails(cx)),
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
                        color_mode.label(),
                        color_mode != ColorMode::Light,
                        cx.listener(|this, _, _, cx| {
                            this.session.update(cx, |s, cx| s.cycle_color_mode(cx));
                        }),
                    ))
                    .child(toolbar_btn(
                        "commands-btn",
                        "/ Cmds",
                        self.command_popup_open,
                        cx.listener(|this, _, window, cx| this.toggle_command_popup(window, cx)),
                    )),
            )
    }

    fn render_status_toast(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (msg, is_err) = self.session.read(cx).status_message.clone()?;
        Some(
            div()
                .id("status-toast")
                .absolute()
                .bottom(px(14.))
                .right(px(14.))
                .max_w(px(460.))
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1p5()
                .rounded_md()
                .shadow_lg()
                .border_1()
                .bg(if is_err { rgb(0x7f1d1d) } else { rgb(0x14532d) })
                .border_color(if is_err { rgb(0xef4444) } else { rgb(0x22c55e) })
                .text_xs()
                .text_color(rgb(0xffffff))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.status_toast_epoch = this.status_toast_epoch.wrapping_add(1);
                    this.last_seen_status = None;
                    this.session.update(cx, |s, cx| {
                        s.status_message = None;
                        cx.notify();
                    });
                }))
                .child(div().flex_1().min_w_0().truncate().child(msg))
                .child(div().text_color(rgb(0xd4d4d8)).child("x"))
                .into_any_element(),
        )
    }

    fn render_brand_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ver_badge = format!("v{}", env!("CARGO_PKG_VERSION"));
        let menu_row = |id: &'static str,
                        label: &'static str,
                        hint: &str,
                        on_click: Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>| {
            let mut row = div()
                .id(id)
                .w_full()
                .px_3()
                .py_1p5()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .cursor_pointer()
                .hover(|s| s.bg(rgb(0x27272a)))
                .child(div().text_color(rgb(0xf4f4f5)).child(label))
                .on_click(move |ev, window, app| on_click(ev, window, app));
            if !hint.is_empty() {
                row = row.child(
                    div()
                        .text_color(rgb(0xa1a1aa))
                        .child(hint.to_string()),
                );
            }
            row
        };
        let ws_about = cx.entity();
        let ws_demo = cx.entity();
        let ws_save_as = cx.entity();
        let ws_cmds = cx.entity();

        div()
            .id("brand-menu-backdrop")
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_brand_menu(window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .id("brand-menu-card")
                    .key_context(OVERLAY_POPUP_CONTEXT)
                    .absolute()
                    .top(px(34.))
                    .left(px(10.))
                    .w(px(236.))
                    .py_1()
                    .rounded_md()
                    .bg(rgb(0x18181b))
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .shadow_lg()
                    .text_xs()
                    .text_color(rgb(0xf4f4f5))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(menu_row(
                        "brand-menu-about",
                        "About Pinleaf",
                        &ver_badge,
                        Box::new(move |_, window, app| {
                            ws_about.update(app, |this, cx| this.open_about_popup(window, cx));
                        }),
                    ))
                    .child(div().h(px(1.)).my_1().bg(rgb(0x27272a)))
                    .child(menu_row(
                        "brand-menu-demo",
                        "Open Built-in Demo",
                        "",
                        Box::new(move |_, window, app| {
                            ws_demo.update(app, |this, cx| {
                                this.close_brand_menu(window, cx);
                                this.open_demo_session(cx);
                            });
                        }),
                    ))
                    .child(menu_row(
                        "brand-menu-save-as",
                        "Save PDF As...",
                        "",
                        Box::new(move |_, window, app| {
                            ws_save_as.update(app, |this, cx| {
                                this.close_brand_menu(window, cx);
                                this.trigger_save(true, cx);
                            });
                        }),
                    ))
                    .child(menu_row(
                        "brand-menu-cmds",
                        "Commands & Shortcuts",
                        "/",
                        Box::new(move |_, window, app| {
                            ws_cmds.update(app, |this, cx| {
                                this.brand_menu_open = false;
                                this.toggle_command_popup(window, cx);
                            });
                        }),
                    ))
                    .child(div().h(px(1.)).my_1().bg(rgb(0x27272a)))
                    .child(menu_row(
                        "brand-menu-quit",
                        "Quit Pinleaf",
                        "Cmd+Q",
                        Box::new(|_, _, app| app.quit()),
                    )),
            )
    }

    fn render_about_popup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let app_icon = cx.global::<AppRegistry>().app_icon.clone();
        let version_badge = format!(
            "Version {} ({}-{})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH,
            std::env::consts::OS
        );
        let clipboard_version = format!(
            "Pinleaf v{} ({}-{})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH,
            std::env::consts::OS
        );

        div()
            .id("about-popup-backdrop")
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .bg(rgba(0x09090b88))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_about_popup(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("about-popup-card")
                    .key_context(OVERLAY_POPUP_CONTEXT)
                    .track_focus(&self.about_focus_handle)
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        cx.stop_propagation();
                        if matches!(event.keystroke.key.as_str(), "escape" | "enter") {
                            this.close_about_popup(window, cx);
                        }
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(380.))
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
                            .px_3p5()
                            .py_2()
                            .border_b_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("About Pinleaf"),
                            )
                            .child(
                                div()
                                    .id("about-close-btn")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x27272a))
                                    .hover(|s| s.bg(rgb(0x3f3f46)))
                                    .cursor_pointer()
                                    .text_color(rgb(0xe4e4e7))
                                    .child("Close [Esc]")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_about_popup(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .px_5()
                            .py_5()
                            .child(img(app_icon).size(px(56.)))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("Pinleaf"),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(rgb(0x1e3a8a))
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xe0f2fe))
                                    .child(version_badge),
                            )
                            .child(
                                div()
                                    .pt_1()
                                    .text_xs()
                                    .text_color(rgb(0xd4d4d8))
                                    .child("Freeze-and-branch multi-view academic PDF reader"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xa1a1aa))
                                    .child("Pure Rust + Metal GPU  |  gpui-ce  |  hayro  |  lopdf"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x71717a))
                                    .child("MIT License"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .px_4()
                            .py_2p5()
                            .border_t_1()
                            .border_color(rgb(0x27272a))
                            .text_xs()
                            .child(
                                div()
                                    .id("about-copy-btn")
                                    .px_2p5()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x27272a))
                                    .hover(|s| s.bg(rgb(0x3f3f46)))
                                    .cursor_pointer()
                                    .text_color(rgb(0xf4f4f5))
                                    .child("Copy Version")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            clipboard_version.clone(),
                                        ));
                                        this.session.update(cx, |s, cx| {
                                            s.status_message = Some((
                                                "Copied Pinleaf version to clipboard".to_string(),
                                                false,
                                            ));
                                            cx.notify();
                                        });
                                    })),
                            )
                            .child(
                                div()
                                    .id("about-github-btn")
                                    .px_2p5()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x27272a))
                                    .hover(|s| s.bg(rgb(0x3f3f46)))
                                    .cursor_pointer()
                                    .text_color(rgb(0xf4f4f5))
                                    .child("GitHub")
                                    .on_click(|_, _, _| {
                                        let _ = crate::external::open_external_url(
                                            "https://github.com/zhanxw/pinleaf",
                                        );
                                    }),
                            )
                            .child(
                                div()
                                    .id("about-ok-btn")
                                    .px_3()
                                    .py_1()
                                    .rounded_sm()
                                    .bg(rgb(0x2563eb))
                                    .hover(|s| s.bg(rgb(0x1d4ed8)))
                                    .cursor_pointer()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("OK")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_about_popup(window, cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let all_items = self
            .session
            .read(cx)
            .annotations
            .all_sorted()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        let mut cat_counts: Vec<(char, usize)> = Vec::new();
        for ann in &all_items {
            if let Some(c) = ann.category {
                if let Some(entry) = cat_counts.iter_mut().find(|(ch, _)| *ch == c) {
                    entry.1 += 1;
                } else {
                    cat_counts.push((c, 1));
                }
            }
        }
        cat_counts.sort_by_key(|(c, _)| *c);

        let total_count = all_items.len();
        let active_filter = self.sidebar_category_filter;
        let items: Vec<_> = match active_filter {
            Some(cat) => all_items
                .into_iter()
                .filter(|a| a.category == Some(cat))
                .collect(),
            None => all_items,
        };

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
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x18181b))
                            .child(format!("Annotations ({}/{})", items.len(), total_count)),
                    )
                    .when(!cat_counts.is_empty(), |hdr| {
                        hdr.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_1()
                                .child(
                                    div()
                                        .id("sb-cat-all")
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_xs()
                                        .bg(if active_filter.is_none() {
                                            rgb(0x2563eb)
                                        } else {
                                            rgb(0xe4e4e7)
                                        })
                                        .text_color(if active_filter.is_none() {
                                            rgb(0xffffff)
                                        } else {
                                            rgb(0x27272a)
                                        })
                                        .child("All")
                                        .on_click(cx.listener(|ws, _, _, cx| {
                                            ws.sidebar_category_filter = None;
                                            cx.notify();
                                        })),
                                )
                                .children(cat_counts.into_iter().map(|(cat, count)| {
                                    let is_active = active_filter == Some(cat);
                                    let (lbl, _) = highlight_category_spec(cat);
                                    div()
                                        .id(("sb-cat", cat as u64))
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_xs()
                                        .bg(if is_active {
                                            rgb(0x2563eb)
                                        } else {
                                            rgb(0xe4e4e7)
                                        })
                                        .text_color(if is_active {
                                            rgb(0xffffff)
                                        } else {
                                            rgb(0x27272a)
                                        })
                                        .child(format!("{cat}:{lbl} ({count})"))
                                        .on_click(cx.listener(move |ws, _, _, cx| {
                                            ws.sidebar_category_filter = if is_active {
                                                None
                                            } else {
                                                Some(cat)
                                            };
                                            cx.notify();
                                        }))
                                })),
                        )
                    }),
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
                                .child("Drag over text to highlight, or press H + [a-z] for a typed category."),
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
                        let cat_badge = ann.category.map(|c| {
                            let (lbl, _) = highlight_category_spec(c);
                            format!("[{c}: {lbl}]")
                        });
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
                                                    .child(match cat_badge {
                                                        Some(b) => format!("{kind_label} {b}"),
                                                        None => kind_label.to_string(),
                                                    }),
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

    fn collect_thumb_page_metas(&self, cx: &App) -> Rc<Vec<ThumbPageMeta>> {
        let sess = self.session.read(cx);
        let count = sess.page_count();
        let active_page = self.active_viewport_entity(cx).read(cx).anchor.page;

        let mut frozen_pages: HashMap<usize, usize> = HashMap::new();
        let mut viewports = Vec::new();
        self.root.all_viewports(&mut viewports);
        let reg = cx.global::<AppRegistry>();
        for vp_ent in viewports {
            let vp = vp_ent.read(cx);
            if vp.freeze_state.is_frozen() {
                let (trail_num, _) = reg.trail_info(vp.id);
                frozen_pages
                    .entry(vp.anchor.page.as_usize())
                    .or_insert(trail_num as usize);
            }
        }

        let mut annot_counts = vec![0usize; count];
        for ann in sess.annotations.all_sorted() {
            if let Some(slot) = annot_counts.get_mut(ann.page.as_usize()) {
                *slot += 1;
            }
        }
        let mut portal_counts = vec![0usize; count];
        for portal in &sess.portals {
            if let Some(slot) = portal_counts.get_mut(portal.src_page.as_usize()) {
                *slot += 1;
            }
        }

        let metas = sess
            .page_metas
            .iter()
            .enumerate()
            .map(|(i, pm)| ThumbPageMeta {
                page: PageIndex(i as u32),
                norm_w: pm.norm_width,
                norm_h: pm.norm_height,
                is_active: active_page.as_usize() == i,
                frozen_trail_num: frozen_pages.get(&i).copied(),
                annot_count: annot_counts[i],
                portal_count: portal_counts[i],
            })
            .collect();
        Rc::new(metas)
    }

    fn render_thumb_card_inner(
        meta: &ThumbPageMeta,
        thumb_img: Option<Arc<RenderImage>>,
        max_w: f32,
        max_h: f32,
        is_selected: bool,
    ) -> gpui::Div {
        let scale = (max_w / meta.norm_w.max(1.0)).min(max_h / meta.norm_h.max(1.0));
        let card_w = (meta.norm_w * scale).round().max(48.0);
        let card_h = (meta.norm_h * scale).round().max(48.0);
        let highlighted = is_selected || meta.is_active;

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .child(
                div()
                    .relative()
                    .w(px(max_w))
                    .h(px(max_h))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .relative()
                            .w(px(card_w))
                            .h(px(card_h))
                            .rounded_sm()
                            .overflow_hidden()
                            .bg(rgb(0xffffff))
                            .border_2()
                            .border_color(if highlighted {
                                rgb(0x2563eb)
                            } else if meta.frozen_trail_num.is_some() {
                                rgb(0xf59e0b)
                            } else {
                                rgb(0xd4d4d8)
                            })
                            .shadow_xs()
                            .child(match thumb_img {
                                Some(t) => img(t).size_full().into_any_element(),
                                None => div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(rgb(0xa1a1aa))
                                    .child(format!("p.{}", meta.page.one_based()))
                                    .into_any_element(),
                            })
                            .when_some(meta.frozen_trail_num, |card, trail_num| {
                                card.child(
                                    div()
                                        .absolute()
                                        .top(px(3.))
                                        .right(px(3.))
                                        .px_1()
                                        .rounded_sm()
                                        .bg(rgb(0xf59e0b))
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0x18181b))
                                        .child(format!("#{trail_num} LOCK")),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .text_xs()
                    .font_weight(if highlighted {
                        FontWeight::BOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(if highlighted {
                        rgb(0x1d4ed8)
                    } else {
                        rgb(0x52525b)
                    })
                    .child(format!("{}", meta.page.one_based()))
                    .when(meta.annot_count > 0, |row| {
                        row.child(div().size(px(6.)).rounded_full().bg(rgb(0x3b82f6)))
                    })
                    .when(meta.portal_count > 0, |row| {
                        row.child(div().size(px(6.)).rounded_full().bg(rgb(0x10b981)))
                    }),
            )
    }

    fn render_thumbnails_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let metas = self.collect_thumb_page_metas(cx);
        let page_count = metas.len();
        let session = self.session.clone();
        let ws_entity = cx.entity().clone();

        div()
            .w(px(196.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(0xf4f4f5))
            .border_r_1()
            .border_color(rgb(0xd4d4d8))
            .child(
                div()
                    .px_2p5()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0xe4e4e7))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x18181b))
                            .child(format!("Pages ({page_count})")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .id("thumbs-open-grid")
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .bg(rgb(0xe4e4e7))
                                    .hover(|s| s.bg(rgb(0xd4d4d8)))
                                    .text_xs()
                                    .text_color(rgb(0x27272a))
                                    .child("Grid")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_thumbnail_grid(window, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("thumbs-close")
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(0xe4e4e7)))
                                    .text_xs()
                                    .text_color(rgb(0x52525b))
                                    .child("×")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_thumbnails(cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list("thumbnails-strip", page_count, move |range, _window, cx| {
                            range
                                .map(|ix| {
                                    let meta = &metas[ix];
                                    let thumb_img = session.update(cx, |s, cx| {
                                        s.get_or_request_thumbnail(meta.page, cx)
                                    });
                                    let ws = ws_entity.clone();
                                    let target_page = meta.page;
                                    div()
                                        .id(("thumb-strip-item", ix))
                                        .w_full()
                                        .h(px(210.))
                                        .py_2()
                                        .px_2()
                                        .cursor_pointer()
                                        .bg(if meta.is_active {
                                            rgba(0x2563eb1a)
                                        } else {
                                            rgba(0x00000000)
                                        })
                                        .hover(|s| s.bg(rgba(0x2563eb14)))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .on_click(move |ev, window, cx| {
                                            let split_new = ev.modifiers().platform;
                                            let _ = ws.update(cx, |this, cx| {
                                                this.jump_to_page_from_ui(target_page, split_new, cx);
                                                this.restore_viewport_focus(window, cx);
                                            });
                                        })
                                        .child(Self::render_thumb_card_inner(
                                            meta,
                                            thumb_img,
                                            140.0,
                                            166.0,
                                            meta.is_active,
                                        ))
                                })
                                .collect()
                        })
                        .track_scroll(&self.thumbs_scroll_handle)
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .px_2p5()
                    .py_1p5()
                    .border_t_1()
                    .border_color(rgb(0xe4e4e7))
                    .text_xs()
                    .text_color(rgb(0x71717a))
                    .child("Click: Jump • ⌘+Click: Split"),
            )
    }

    fn handle_thumbnail_grid_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let ks = &event.keystroke;
        let count = self.session.read(cx).page_count();
        if ks.key == "escape" || count == 0 {
            self.close_thumbnail_grid(window, cx);
            return;
        }
        let last = count - 1;
        if ks.key == "enter" {
            let split_new = ks.modifiers.platform;
            let target = PageIndex(self.thumbnail_grid_selected.min(last) as u32);
            self.close_thumbnail_grid(window, cx);
            self.jump_to_page_from_ui(target, split_new, cx);
            return;
        }
        let cur = self.thumbnail_grid_selected.min(last);
        let next = match ks.key.as_str() {
            "left" | "h" => Some(cur.saturating_sub(1)),
            "right" | "l" => Some((cur + 1).min(last)),
            "up" | "k" => Some(cur.saturating_sub(4)),
            "down" | "j" => Some((cur + 4).min(last)),
            "home" => Some(0),
            "end" => Some(last),
            "g" => Some(if ks.modifiers.shift { last } else { 0 }),
            _ => None,
        };
        if let Some(new_idx) = next {
            self.thumbnail_grid_selected = new_idx;
            self.thumbnail_grid_scroll_handle
                .scroll_to_item(new_idx / 4, ScrollStrategy::Nearest);
            cx.notify();
        }
    }

    fn render_thumbnail_grid_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let metas = self.collect_thumb_page_metas(cx);
        let page_count = metas.len();
        let row_count = page_count.div_ceil(4);
        let selected = self.thumbnail_grid_selected.min(page_count.saturating_sub(1));
        let session = self.session.clone();
        let ws_entity = cx.entity().clone();

        div()
            .id("thumb-grid-backdrop")
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .bg(rgba(0x09090ba0))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_thumbnail_grid(window, cx);
                    cx.stop_propagation();
                }),
            )
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("thumb-grid-card")
                    .key_context(OVERLAY_POPUP_CONTEXT)
                    .track_focus(&self.thumbnail_grid_focus_handle)
                    .on_key_down(cx.listener(Self::handle_thumbnail_grid_key_down))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(860.))
                    .h(px(620.))
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(rgb(0xfafafa))
                    .border_1()
                    .border_color(rgb(0xd4d4d8))
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .px_4()
                            .py_2p5()
                            .bg(rgb(0x18181b))
                            .text_color(rgb(0xf4f4f5))
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_xs()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!("Thumbnail Grid Overview ({page_count} pages)")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_color(rgb(0xa1a1aa))
                                            .child("Arrows/hjkl Move · Enter Jump · Cmd+Enter Split · Esc Close"),
                                    )
                                    .child(
                                        div()
                                            .id("thumb-grid-close")
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(rgb(0x27272a))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x3f3f46)))
                                            .child("Close")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.close_thumbnail_grid(window, cx);
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(
                                uniform_list(
                                    "thumbnails-grid",
                                    row_count,
                                    move |range, _window, cx| {
                                        range
                                            .map(|row_ix| {
                                                let mut cols = Vec::with_capacity(4);
                                                for col_ix in 0..4 {
                                                    let ix = row_ix * 4 + col_ix;
                                                    if ix < page_count {
                                                        let meta = &metas[ix];
                                                        let thumb_img = session.update(cx, |s, cx| {
                                                            s.get_or_request_thumbnail(meta.page, cx)
                                                        });
                                                        let ws = ws_entity.clone();
                                                        let is_sel = ix == selected;
                                                        let target_page = meta.page;
                                                        cols.push(
                                                            div()
                                                                .id(("thumb-grid-cell", ix))
                                                                .flex_1()
                                                                .h_full()
                                                                .rounded_sm()
                                                                .cursor_pointer()
                                                                .bg(if is_sel {
                                                                    rgba(0x2563eb1a)
                                                                } else {
                                                                    rgba(0x00000000)
                                                                })
                                                                .hover(|s| s.bg(rgba(0x2563eb12)))
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .on_click(move |ev, window, cx| {
                                                                    let split_new =
                                                                        ev.modifiers().platform;
                                                                    let _ = ws.update(cx, |this, cx| {
                                                                        this.thumbnail_grid_selected =
                                                                            ix;
                                                                        this.close_thumbnail_grid(
                                                                            window, cx,
                                                                        );
                                                                        this.jump_to_page_from_ui(
                                                                            target_page, split_new,
                                                                            cx,
                                                                        );
                                                                    });
                                                                })
                                                                .child(Self::render_thumb_card_inner(
                                                                    meta,
                                                                    thumb_img,
                                                                    172.0,
                                                                    192.0,
                                                                    is_sel,
                                                                ))
                                                                .into_any_element(),
                                                        );
                                                    } else {
                                                        cols.push(
                                                            div()
                                                                .flex_1()
                                                                .h_full()
                                                                .into_any_element(),
                                                        );
                                                    }
                                                }
                                                div()
                                                    .id(("thumb-grid-row", row_ix))
                                                    .w_full()
                                                    .h(px(236.))
                                                    .px_3()
                                                    .py_1p5()
                                                    .flex()
                                                    .flex_row()
                                                    .gap_3()
                                                    .children(cols)
                                            })
                                            .collect()
                                    },
                                )
                                .track_scroll(&self.thumbnail_grid_scroll_handle)
                                .size_full(),
                            ),
                    ),
            )
    }

    fn handle_goto_page_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let ks = &event.keystroke;
        if ks.key == "escape" {
            self.close_goto_page_popup(window, cx);
            return;
        }
        if ks.key == "backspace" {
            self.goto_page_query.pop();
            cx.notify();
            return;
        }
        if ks.key == "enter" {
            let page_count = self.session.read(cx).page_count();
            if page_count > 0
                && let Ok(parsed) = self.goto_page_query.trim().parse::<u32>()
                && parsed >= 1
            {
                let clamped = parsed.clamp(1, page_count as u32);
                let split_new = ks.modifiers.platform;
                self.close_goto_page_popup(window, cx);
                self.jump_to_page_from_ui(PageIndex(clamped - 1), split_new, cx);
            } else {
                self.close_goto_page_popup(window, cx);
            }
            return;
        }
        if !ks.modifiers.platform
            && !ks.modifiers.control
            && !ks.modifiers.alt
            && let Some(ch_str) = &ks.key_char
            && let Some(ch) = ch_str.chars().next()
            && ch.is_ascii_digit()
            && (!self.goto_page_query.is_empty() || ch != '0')
            && self.goto_page_query.len() < 5
        {
            self.goto_page_query.push(ch);
            cx.notify();
        }
    }

    fn render_goto_page_popup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let page_count = self.session.read(cx).page_count();
        let current_page = self.active_viewport_entity(cx).read(cx).anchor.page.one_based();
        let display_query = if self.goto_page_query.is_empty() {
            format!("Page 1–{page_count} (now {current_page})|")
        } else {
            format!("{}|", self.goto_page_query)
        };
        let is_empty = self.goto_page_query.is_empty();

        div()
            .id("goto-page-backdrop")
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .bg(rgba(0x09090b77))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(88.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.close_goto_page_popup(window, cx);
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .id("goto-page-card")
                    .key_context(OVERLAY_POPUP_CONTEXT)
                    .track_focus(&self.goto_page_focus_handle)
                    .on_key_down(cx.listener(Self::handle_goto_page_key_down))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(px(340.))
                    .p_3()
                    .rounded_md()
                    .bg(rgb(0x18181b))
                    .border_1()
                    .border_color(rgb(0x3f3f46))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_xs()
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("Go to Page"),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xa1a1aa))
                                    .child("Enter Jump · Cmd+Enter Split · Esc"),
                            ),
                    )
                    .child(
                        div()
                            .px_2p5()
                            .py_1p5()
                            .rounded_sm()
                            .bg(rgb(0x09090b))
                            .border_1()
                            .border_color(rgb(0x2563eb))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if is_empty {
                                rgb(0x71717a)
                            } else {
                                rgb(0xffffff)
                            })
                            .child(display_query),
                    ),
            )
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
        let page_count = self.session.read(cx).page_count();
        let page_jump = parse_page_jump_query(&self.command_query, page_count);
        let matches = filter_commands(&self.command_query);
        let match_len = matches.len();
        let total_rows = match_len + usize::from(page_jump.is_some());
        let selected_idx = if total_rows > 0 {
            Some(self.selected_command_idx.min(total_rows - 1))
        } else {
            None
        };
        let is_unfiltered = self.command_query.trim().is_empty();

        let mut list_items: Vec<AnyElement> = Vec::new();
        if let Some(target_page_1based) = page_jump {
            let is_sel = selected_idx == Some(0);
            list_items.push(
                div()
                    .id("cmd-row-page-jump")
                    .px_2p5()
                    .py_1p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .bg(if is_sel { rgb(0x1e3a8a) } else { rgb(0x18181b) })
                    .border_1()
                    .border_color(if is_sel { rgb(0x3b82f6) } else { rgb(0x27272a) })
                    .hover(|s| s.bg(rgb(0x1d4ed8)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.close_command_popup(window, cx);
                        this.jump_to_page_from_ui(PageIndex(target_page_1based - 1), false, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .child(format!("Jump to Page {target_page_1based} of {page_count}")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x93c5fd))
                                    .child("Direct page navigation"),
                            ),
                    )
                    .child(
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(rgb(0x27272a))
                            .text_xs()
                            .text_color(rgb(0xe4e4e7))
                            .child("Enter"),
                    )
                    .into_any_element(),
            );
        }
        let cmd_offset = usize::from(page_jump.is_some());
        if matches.is_empty() && page_jump.is_none() {
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
                let row_idx = idx + cmd_offset;
                list_items.push(self.render_command_row(
                    idx,
                    entry,
                    selected_idx == Some(row_idx),
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
                                            .child(format!("{total_rows}")),
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
                                            .child("Up/Down Navigate · Enter Run · Esc Close"),
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
            LibraryModalKind::Extensions => (
                "Extensions & External Commands",
                "Filter configured shell/Python extensions and tools...",
                "No matching extensions.",
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
                let item_click = item.clone();
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
                            this.activate_library_item(&item_click, window, cx);
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

    pub fn focus_synctex_forward(
        &mut self,
        tex_path: &std::path::Path,
        line: u32,
        cx: &mut Context<Self>,
    ) -> bool {
        let loc_opt = self
            .session
            .update(cx, |s, cx| s.apply_synctex_forward(tex_path, line, cx));
        if let Some(loc) = loc_opt {
            self.navigate_focused_or_smart_route(loc.page, (loc.rect.min_y - 48.0).max(0.0), cx);
            true
        } else {
            false
        }
    }

    pub fn execute_ipc_command(
        &mut self,
        command: &str,
        args: &[String],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let vp = self.active_viewport_entity(cx);
        let vp_id = vp.read(cx).id;
        match command {
            "next_page" => vp.update(cx, |vp, cx| vp.next_page(cx)),
            "prev_page" => vp.update(cx, |vp, cx| vp.prev_page(cx)),
            "first_page" => vp.update(cx, |vp, cx| vp.go_first_page(cx)),
            "last_page" => vp.update(cx, |vp, cx| vp.go_last_page(cx)),
            "goto_page" => {
                let page_num: u32 = args
                    .first()
                    .ok_or_else(|| "goto_page requires 1-based page number".to_string())?
                    .parse()
                    .map_err(|e| format!("invalid page number: {e}"))?;
                self.navigate_focused_or_smart_route(
                    PageIndex(page_num.saturating_sub(1)),
                    0.0,
                    cx,
                );
            }
            "zoom_in" => vp.update(cx, |vp, cx| vp.zoom_in(cx)),
            "zoom_out" => vp.update(cx, |vp, cx| vp.zoom_out(cx)),
            "fit_width" => vp.update(cx, |vp, cx| vp.zoom_fit_width(cx)),
            "fit_content" => vp.update(cx, |vp, cx| vp.zoom_fit_content_width(cx)),
            "cycle_color_mode" => self.session.update(cx, |s, cx| s.cycle_color_mode(cx)),
            "toggle_smart_dark" => self.session.update(cx, |s, cx| s.toggle_smart_dark_mode(cx)),
            "toggle_freeze" => vp.update(cx, |vp, cx| vp.toggle_freeze(cx)),
            "split_right" => {
                self.freeze_and_split(vp_id, Axis::Horizontal, cx);
            }
            "split_down" => {
                self.freeze_and_split(vp_id, Axis::Vertical, cx);
            }
            "toggle_portal_follower" => self.toggle_portal_follower(vp_id, cx),
            "toggle_thumbnails" => self.toggle_thumbnails(cx),
            "toggle_thumbnail_grid" => self.toggle_thumbnail_grid(window, cx),
            "toggle_presentation" => self.toggle_presentation_mode(window, cx),
            "toggle_autoscroll" => vp.update(cx, |vp, cx| vp.toggle_auto_scroll(cx)),
            "search" => {
                let query = args.join(" ");
                self.session.update(cx, |s, cx| s.update_search(query, false, cx));
                if let Some((p, y)) = self.session.read(cx).current_search_match() {
                    self.navigate_focused_or_smart_route(p, (y - 36.0).max(0.0), cx);
                }
            }
            "reload" => {
                self.session.update(cx, |s, cx| {
                    s.force_reload_from_disk(cx);
                });
            }
            "about" => {
                self.brand_menu_open = false;
                self.about_popup_open = true;
                self.pending_focus = Some(self.about_focus_handle.clone());
                cx.notify();
            }
            "quit" => cx.quit(),
            other => return Err(format!("unknown IPC command: {other}")),
        }
        Ok(format!("executed {command}"))
    }

    fn handle_workspace_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.presentation_mode
            && event.keystroke.key == "escape"
            && !self.command_popup_open
            && !self.toc_popup_open
            && !self.goto_page_open
            && !self.thumbnail_grid_open
            && !self.about_popup_open
            && !self.brand_menu_open
            && self.library_modal.is_none()
            && !self.search_bar_open
        {
            self.toggle_presentation_mode(window, cx);
            cx.stop_propagation();
            return;
        }

        if self.brand_menu_open {
            if event.keystroke.key == "escape" {
                self.close_brand_menu(window, cx);
                cx.stop_propagation();
            }
            return;
        }

        if self.command_popup_open
            || self.toc_popup_open
            || self.goto_page_open
            || self.thumbnail_grid_open
            || self.about_popup_open
            || self.library_modal.is_some()
            || self.search_bar_open
        {
            return;
        }

        let matched_cmd = {
            let exts = &cx.global::<AppRegistry>().user_config.extensions;
            exts.iter().find_map(|ext| {
                let raw_key = ext.key.as_deref()?;
                let parsed = Keystroke::parse(raw_key).ok()?;
                if parsed.key == event.keystroke.key && parsed.modifiers == event.keystroke.modifiers
                {
                    Some(ext.command.clone())
                } else {
                    None
                }
            })
        };
        if let Some(cmd) = matched_cmd {
            self.run_extension_command(cmd, cx);
            cx.stop_propagation();
        }
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

        let (doc_title, doc_dirty) = {
            let s = self.session.read(cx);
            (s.title.clone(), s.annotations.is_dirty())
        };
        let desired_title = if doc_dirty {
            format!("Pinleaf — {doc_title} *")
        } else {
            format!("Pinleaf — {doc_title}")
        };
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
            .on_action(cx.listener(|this, _: &ShowAbout, window, cx| this.open_about_popup(window, cx)))
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
            .on_action(cx.listener(|this, _: &GoToPage, window, cx| {
                this.open_goto_page_popup(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleThumbnails, _, cx| {
                this.toggle_thumbnails(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleThumbnailGrid, window, cx| {
                this.toggle_thumbnail_grid(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenRecentPopup, window, cx| {
                this.toggle_library_modal(LibraryModalKind::RecentDocs, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenGlobalHighlightsPopup, window, cx| {
                this.toggle_library_modal(LibraryModalKind::GlobalHighlights, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenExtensionsPopup, window, cx| {
                this.toggle_library_modal(LibraryModalKind::Extensions, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleAutoReload, _, cx| {
                this.session.update(cx, |s, cx| s.toggle_auto_reload(cx));
            }))
            .on_action(cx.listener(|this, _: &CycleColorMode, _, cx| {
                this.session.update(cx, |s, cx| s.cycle_color_mode(cx));
            }))
            .on_action(cx.listener(|this, _: &ToggleSmartDarkMode, _, cx| {
                this.session.update(cx, |s, cx| s.toggle_smart_dark_mode(cx));
            }))
            .on_action(cx.listener(|this, _: &TogglePresentationMode, window, cx| {
                this.toggle_presentation_mode(window, cx);
            }))
            .on_key_down(cx.listener(Self::handle_workspace_key_down))
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
            .when(!self.presentation_mode, |root| {
                root.child(self.render_toolbar(cx))
            })
            .when(self.search_bar_open, |root| {
                root.child(self.render_search_bar(cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .when(self.thumbnails_open && !self.presentation_mode, |row| {
                        row.child(self.render_thumbnails_strip(cx))
                    })
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(tree_view)
                            .children(self.render_status_toast(cx)),
                    )
                    .when(self.sidebar_open, |row| row.child(self.render_sidebar(cx))),
            )
            .when(self.toc_popup_open, |root| root.child(self.render_toc_popup(cx)))
            .when_some(self.library_modal, |root, kind| {
                root.child(self.render_library_modal(kind, cx))
            })
            .when(self.thumbnail_grid_open, |root| {
                root.child(self.render_thumbnail_grid_modal(cx))
            })
            .when(self.goto_page_open, |root| {
                root.child(self.render_goto_page_popup(cx))
            })
            .when(self.command_popup_open, |root| {
                root.child(self.render_command_popup(cx))
            })
            .when(self.brand_menu_open && !self.presentation_mode, |root| {
                root.child(self.render_brand_menu(cx))
            })
            .when(self.about_popup_open, |root| {
                root.child(self.render_about_popup(cx))
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
        .px_2()
        .py_0p5()
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
