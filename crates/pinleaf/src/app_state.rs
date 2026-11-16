use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{AnyWindowHandle, App, BorrowAppContext, Entity, Global, RenderImage, WeakEntity, Window};
use pinleaf_core::{
    FreezeState, PageIndex, ReadingTrailGraph, RgbaColor, TrailNode, ViewportId,
    find_active_portal, highlight_category_spec,
};

use crate::library::{
    IndexedHighlight, LibraryStore, RecentDocEntry, UserConfig, now_unix_secs, parse_hex_rgb,
};
use crate::session::DocumentSession;
use crate::viewport::PdfViewport;

#[derive(Clone)]
pub struct ViewportRegistryEntry {
    pub viewport: WeakEntity<PdfViewport>,
    pub window: AnyWindowHandle,
}

/// Global coordinator shared across every OS window:
/// - Ensures each file path maps to one shared `Entity<DocumentSession>`.
/// - Tracks the unlimited `ReadingTrailGraph` across split panes and OS windows.
/// - Holds user preferences and cross-PDF recent library / highlight index.
pub struct AppRegistry {
    sessions_by_path: HashMap<PathBuf, WeakEntity<DocumentSession>>,
    demo_session: Option<WeakEntity<DocumentSession>>,
    pub trail_graph: ReadingTrailGraph,
    pub viewports: HashMap<ViewportId, ViewportRegistryEntry>,
    pub flash_viewport: Option<ViewportId>,
    pub app_icon: Arc<RenderImage>,
    pub library: LibraryStore,
    pub user_config: UserConfig,
}

impl Global for AppRegistry {}

impl AppRegistry {
    pub fn init(cx: &mut App) {
        cx.set_global(Self {
            sessions_by_path: HashMap::new(),
            demo_session: None,
            trail_graph: ReadingTrailGraph::new(),
            viewports: HashMap::new(),
            flash_viewport: None,
            app_icon: crate::icon::build_gpui_app_icon(64),
            library: LibraryStore::load(),
            user_config: UserConfig::load(),
        });
    }

    pub fn prepare_session_model(&self, session: &mut DocumentSession) {
        if let Some(mode) = self.user_config.default_color_mode {
            session.color_mode = mode;
        }
        if let Some(smart) = self.user_config.smart_dark_mode {
            session.smart_dark_mode = smart;
        }
        if let Some(ref bg) = self.user_config.custom_bg_color
            && let Some(rgb) = parse_hex_rgb(bg)
        {
            session.custom_bg_rgb = rgb;
        }
        if let Some(ref fg) = self.user_config.custom_fg_color
            && let Some(rgb) = parse_hex_rgb(fg)
        {
            session.custom_fg_rgb = rgb;
        }
        if let Some(auto_reload) = self.user_config.auto_reload {
            session.auto_reload = auto_reload;
        }
        if let Some(ref path) = session.path {
            let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            if let Some(entry) = self
                .library
                .documents
                .iter()
                .find(|d| d.path == canonical || &d.path == path)
                && !entry.portals.is_empty()
            {
                session.portals = entry.portals.clone();
                session.next_portal_id = session
                    .portals
                    .iter()
                    .map(|p| p.id)
                    .max()
                    .unwrap_or(0)
                    + 1;
            }
        }
    }

    pub fn resolve_highlight_category(&self, ch: char) -> (String, RgbaColor) {
        let lower = ch.to_ascii_lowercase();
        let key = lower.to_string();
        if let Some(custom) = self.user_config.highlight_categories.get(&key) {
            let (_, default_color) = highlight_category_spec(lower);
            let color = parse_hex_rgb(&custom.color)
                .map(|[r, g, b]| {
                    RgbaColor::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
                })
                .unwrap_or(default_color);
            let label = if custom.label.trim().is_empty() {
                highlight_category_spec(lower).0.to_string()
            } else {
                custom.label.clone()
            };
            (label, color)
        } else {
            let (label, color) = highlight_category_spec(lower);
            (label.to_string(), color)
        }
    }

    pub fn sync_session_to_library(&mut self, session: &DocumentSession, last_page: u32) {
        let Some(ref path) = session.path else {
            return;
        };
        let highlights = session
            .annotations
            .all_sorted()
            .into_iter()
            .filter_map(|ann| {
                let quote = ann.selected_text.clone().unwrap_or_default();
                let note = ann.note_contents.clone().unwrap_or_default();
                if quote.is_empty() && note.is_empty() {
                    None
                } else {
                    Some(IndexedHighlight {
                        page_one_based: ann.page.one_based(),
                        quote,
                        note,
                    })
                }
            })
            .collect();

        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.library.upsert_entry(RecentDocEntry {
            path: canonical,
            title: session.title.clone(),
            last_opened_secs: now_unix_secs(),
            page_count: session.page_count(),
            last_page,
            highlights,
            portals: session.portals.clone(),
        });
        let _ = self.library.save();
    }

    pub fn register_and_start_session(
        session: &Entity<DocumentSession>,
        last_page: u32,
        cx: &mut App,
    ) {
        session.update(cx, |s, cx| {
            s.start_file_watcher(cx);
            s.trigger_background_text_index(cx);
        });
        let snapshot_path = session.read(cx).path.clone();
        if let Some(ref path) = snapshot_path {
            cx.add_recent_document(path);
        }
        cx.update_global::<AppRegistry, _>(|reg, cx_inner| {
            reg.register_session(snapshot_path.as_deref(), session);
            let s_ref = session.read(cx_inner);
            reg.sync_session_to_library(s_ref, last_page);
        });
    }

    pub fn register_session(
        &mut self,
        path: Option<&Path>,
        session: &Entity<DocumentSession>,
    ) {
        if let Some(p) = path {
            let key = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
            self.sessions_by_path.insert(key, session.downgrade());
        } else {
            self.demo_session = Some(session.downgrade());
        }
    }

    pub fn lookup_session(&self, path: &Path) -> Option<Entity<DocumentSession>> {
        let key = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.sessions_by_path.get(&key).and_then(WeakEntity::upgrade)
    }

    pub fn register_viewport(
        &mut self,
        parent: Option<ViewportId>,
        freeze_state: FreezeState,
        viewport: &Entity<PdfViewport>,
        window: AnyWindowHandle,
    ) -> TrailNode {
        let node = self.trail_graph.register_child(parent, freeze_state);
        self.viewports.insert(
            node.id,
            ViewportRegistryEntry {
                viewport: viewport.downgrade(),
                window,
            },
        );
        node
    }

    pub fn update_viewport_window(&mut self, id: ViewportId, window: AnyWindowHandle) {
        if let Some(entry) = self.viewports.get_mut(&id) {
            entry.window = window;
        }
    }

    pub fn set_freeze_state(&mut self, id: ViewportId, freeze_state: FreezeState) {
        self.trail_graph.set_freeze_state(id, freeze_state);
    }

    pub fn unregister_viewport(&mut self, id: ViewportId) {
        self.trail_graph.remove(id);
        self.viewports.remove(&id);
        if self.flash_viewport == Some(id) {
            self.flash_viewport = None;
        }
    }

    pub fn trail_info(&self, id: ViewportId) -> (u32, Option<u32>) {
        let Some(node) = self.trail_graph.get(id) else {
            return (1, None);
        };
        let parent_num = node
            .parent
            .and_then(|pid| self.trail_graph.get(pid))
            .map(|p| p.trail_number);
        (node.trail_number, parent_num)
    }

    pub fn parent_id(&self, id: ViewportId) -> Option<ViewportId> {
        self.trail_graph.get(id).and_then(|n| n.parent)
    }

    /// Finds an active descendant viewport (in any window) to smart-route a link clicked
    /// inside a frozen viewport.
    pub fn find_smart_route_target(&self, source: ViewportId) -> Option<ViewportId> {
        self.trail_graph.find_active_descendant(source)
    }
}

/// Navigates an existing viewport (even if it lives in another OS window) to `(page, top_norm_y)`.
pub fn navigate_viewport_globally(
    target_id: ViewportId,
    page: PageIndex,
    top_norm_y: Option<f32>,
    cx: &mut App,
) -> bool {
    let entry = cx
        .global::<AppRegistry>()
        .viewports
        .get(&target_id)
        .cloned();
    let Some(entry) = entry else {
        return false;
    };
    let Some(vp) = entry.viewport.upgrade() else {
        return false;
    };

    cx.update_global::<AppRegistry, _>(|reg, _| {
        reg.flash_viewport = Some(target_id);
    });

    vp.update(cx, |vp, cx| {
        vp.jump_to_page_offset_recorded(page, top_norm_y.unwrap_or(0.0), cx);
    });

    let _ = entry.window.update(cx, |_, window, _| {
        window.activate();
        window.refresh();
    });
    true
}

/// Highlights & focuses a parent/target viewport across split panes or OS windows.
pub fn focus_viewport_globally(
    target_id: ViewportId,
    current_window: Option<&mut Window>,
    cx: &mut App,
) {
    let entry = cx
        .global::<AppRegistry>()
        .viewports
        .get(&target_id)
        .cloned();
    let Some(entry) = entry else {
        return;
    };
    cx.update_global::<AppRegistry, _>(|reg, _| {
        reg.flash_viewport = Some(target_id);
    });
    if let Some(vp) = entry.viewport.upgrade() {
        vp.update(cx, |_, cx| cx.notify());
    }
    if let Some(win) = current_window
        && win.window_handle() == entry.window
    {
        if let Some(vp) = entry.viewport.upgrade() {
            let fh = vp.read(cx).focus_handle().clone();
            win.focus(&fh, cx);
        }
        return;
    }
        let _ = entry.window.update(cx, |_, window, cx| {
        window.activate();
        if let Some(vp) = entry.viewport.upgrade() {
            let fh = vp.read(cx).focus_handle().clone();
            window.focus(&fh, cx);
        }
        window.refresh();
    });
}

/// Syncs all active `portal_follower` viewports (in splits or separate OS windows) showing
/// `session` to the closest scroll-linked portal for `(cur_page, cur_focal_y)`. Returns how
/// many follower viewports were found.
pub fn sync_portal_followers(
    session: &Entity<DocumentSession>,
    source_id: ViewportId,
    cur_page: PageIndex,
    cur_focal_y: f32,
    cx: &mut App,
) -> usize {
    let active_portal = {
        let s = session.read(cx);
        find_active_portal(&s.portals, cur_page, cur_focal_y).cloned()
    };
    let followers: Vec<(ViewportId, Entity<PdfViewport>, AnyWindowHandle)> = cx
        .global::<AppRegistry>()
        .viewports
        .iter()
        .filter_map(|(&id, entry)| {
            if id == source_id {
                return None;
            }
            let vp = entry.viewport.upgrade()?;
            let vref = vp.read(cx);
            if vref.session == *session && vref.portal_follower {
                Some((id, vp, entry.window))
            } else {
                None
            }
        })
        .collect();

    let count = followers.len();
    if let Some(portal) = active_portal {
        for (_, vp, win) in followers {
            let changed = vp.update(cx, |vp, cx| {
                let target_changed = vp.anchor != portal.dst
                    || vp.active_portal_label.as_deref() != Some(portal.label.as_str());
                if target_changed {
                    vp.anchor = portal.dst;
                    vp.active_portal_label = Some(portal.label.clone());
                    cx.notify();
                }
                target_changed
            });
            if changed {
                let _ = win.update(cx, |_, window, _| window.refresh());
            }
        }
    }
    count
}
