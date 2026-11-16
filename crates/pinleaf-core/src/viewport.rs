use std::collections::{HashMap, VecDeque};

use crate::coords::{NormPoint, PageIndex};

/// Stable unique identifier for an open viewport pane (inside a split tree or standalone window).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViewportId(pub u64);

#[derive(Copy, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ZoomSpec {
    FitWidth,
    FitContentWidth,
    FitPage,
    /// Fixed zoom where `1.0` means 1 PDF point = 1 logical pixel.
    Fixed(f32),
}

impl Default for ZoomSpec {
    fn default() -> Self {
        Self::FitWidth
    }
}

/// Exact reading position within a document, preserved when freezing or cloning a viewport.
#[derive(Copy, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReadingAnchor {
    pub page: PageIndex,
    /// Scroll focal point normalized relative to the page dimensions (`0.0..=1.0`).
    pub focal_norm: NormPoint,
    pub zoom: ZoomSpec,
}

impl Default for ReadingAnchor {
    fn default() -> Self {
        Self {
            page: PageIndex(0),
            focal_norm: NormPoint::new(0.5, 0.0),
            zoom: ZoomSpec::FitWidth,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FreezeState {
    #[default]
    Active,
    Frozen,
}

impl FreezeState {
    pub fn is_frozen(self) -> bool {
        matches!(self, Self::Frozen)
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Active => Self::Frozen,
            Self::Frozen => Self::Active,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrailNode {
    pub id: ViewportId,
    pub trail_number: u32,
    pub parent: Option<ViewportId>,
    pub children: Vec<ViewportId>,
    pub freeze_state: FreezeState,
}

/// Tracks the unlimited "freeze & branch" lineage graph across all split panes and OS windows.
#[derive(Clone, Debug, Default)]
pub struct ReadingTrailGraph {
    next_viewport_raw: u64,
    next_trail_number: u32,
    nodes: HashMap<ViewportId, TrailNode>,
}

impl ReadingTrailGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_root(&mut self, freeze_state: FreezeState) -> TrailNode {
        self.register_child(None, freeze_state)
    }

    pub fn register_child(&mut self, parent: Option<ViewportId>, freeze_state: FreezeState) -> TrailNode {
        self.next_viewport_raw += 1;
        self.next_trail_number += 1;
        let id = ViewportId(self.next_viewport_raw);
        let node = TrailNode {
            id,
            trail_number: self.next_trail_number,
            parent,
            children: Vec::new(),
            freeze_state,
        };
        if let Some(pid) = parent
            && let Some(pnode) = self.nodes.get_mut(&pid)
        {
            pnode.children.push(id);
        }
        self.nodes.insert(id, node.clone());
        node
    }

    pub fn set_freeze_state(&mut self, id: ViewportId, freeze_state: FreezeState) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.freeze_state = freeze_state;
        }
    }

    pub fn get(&self, id: ViewportId) -> Option<&TrailNode> {
        self.nodes.get(&id)
    }

    /// Breadth-first search among descendants for an `Active` viewport to route link clicks
    /// triggered inside a `Frozen` viewport.
    pub fn find_active_descendant(&self, source: ViewportId) -> Option<ViewportId> {
        let start = self.nodes.get(&source)?;
        let mut queue: VecDeque<ViewportId> = start.children.iter().copied().collect();
        while let Some(cid) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&cid) {
                if !node.freeze_state.is_frozen() {
                    return Some(cid);
                }
                queue.extend(node.children.iter().copied());
            }
        }
        None
    }

    /// Removes `id` from the graph, re-parenting any direct children to `id.parent` so the trail remains intact.
    pub fn remove(&mut self, id: ViewportId) {
        let Some(removed) = self.nodes.remove(&id) else {
            return;
        };
        if let Some(pid) = removed.parent
            && let Some(pnode) = self.nodes.get_mut(&pid)
        {
            pnode.children.retain(|&c| c != id);
            for &child_id in &removed.children {
                pnode.children.push(child_id);
            }
        }
        for child_id in removed.children {
            if let Some(cnode) = self.nodes.get_mut(&child_id) {
                cnode.parent = removed.parent;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Directional scroll-linked portal mapping a source reading region to a helper target anchor.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PortalLink {
    pub id: u64,
    pub src_page: PageIndex,
    pub src_focal_y: f32,
    pub dst: ReadingAnchor,
    pub label: String,
}

/// Selects the active portal for the user's current reading position, prioritizing portals
/// at or just above the current reading line.
pub fn find_active_portal(
    portals: &[PortalLink],
    cur_page: PageIndex,
    cur_focal_y: f32,
) -> Option<&PortalLink> {
    if portals.is_empty() {
        return None;
    }
    let cur = cur_page.0 as f32 + cur_focal_y.clamp(0.0, 1.0);
    portals.iter().min_by(|a, b| {
        let score = |portal: &PortalLink| -> f32 {
            let p = portal.src_page.0 as f32 + portal.src_focal_y.clamp(0.0, 1.0);
            let diff = cur - p;
            let raw = diff.abs();
            if (-0.20..=1.20).contains(&diff) {
                raw - 0.30
            } else {
                raw
            }
        };
        score(a).total_cmp(&score(b)).then_with(|| a.id.cmp(&b.id))
    })
}

/// Bounded per-viewport back/forward reading jump stack.
#[derive(Clone, Debug, Default)]
pub struct JumpHistory {
    back: Vec<ReadingAnchor>,
    forward: Vec<ReadingAnchor>,
}

impl JumpHistory {
    const MAX_HISTORY: usize = 64;

    pub fn push_before_jump(&mut self, current: ReadingAnchor) {
        let same_as_top = self.back.last().is_some_and(|prev| {
            prev.page == current.page
                && (prev.focal_norm.y * 100.0).round() == (current.focal_norm.y * 100.0).round()
        });
        if same_as_top {
            return;
        }
        if self.back.len() >= Self::MAX_HISTORY {
            self.back.remove(0);
        }
        self.back.push(current);
        self.forward.clear();
    }

    pub fn go_back(&mut self, current: ReadingAnchor) -> Option<ReadingAnchor> {
        let target = self.back.pop()?;
        if self.forward.len() >= Self::MAX_HISTORY {
            self.forward.remove(0);
        }
        self.forward.push(current);
        Some(target)
    }

    pub fn go_forward(&mut self, current: ReadingAnchor) -> Option<ReadingAnchor> {
        let target = self.forward.pop()?;
        if self.back.len() >= Self::MAX_HISTORY {
            self.back.remove(0);
        }
        self.back.push(current);
        Some(target)
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_generation_branch_and_smart_route() {
        let mut graph = ReadingTrailGraph::new();
        let v1 = graph.register_root(FreezeState::Active);
        assert_eq!(v1.trail_number, 1);

        // Freeze V1 and spawn V2
        graph.set_freeze_state(v1.id, FreezeState::Frozen);
        let v2 = graph.register_child(Some(v1.id), FreezeState::Active);
        assert_eq!(v2.trail_number, 2);
        assert_eq!(graph.find_active_descendant(v1.id), Some(v2.id));

        // Freeze V2 and spawn V3
        graph.set_freeze_state(v2.id, FreezeState::Frozen);
        let v3 = graph.register_child(Some(v2.id), FreezeState::Active);
        assert_eq!(v3.trail_number, 3);
        // Smart link click in V1 routes down to active V3!
        assert_eq!(graph.find_active_descendant(v1.id), Some(v3.id));
        assert_eq!(graph.find_active_descendant(v2.id), Some(v3.id));

        // Closing V2 re-parents V3 to V1 cleanly
        graph.remove(v2.id);
        assert_eq!(graph.get(v3.id).unwrap().parent, Some(v1.id));
        assert_eq!(graph.find_active_descendant(v1.id), Some(v3.id));
    }

    #[test]
    fn jump_history_back_forward_and_dedup() {
        let mut hist = JumpHistory::default();
        let a1 = ReadingAnchor {
            page: PageIndex(0),
            focal_norm: NormPoint::new(0.5, 0.25),
            zoom: ZoomSpec::FitWidth,
        };
        let a2 = ReadingAnchor {
            page: PageIndex(2),
            focal_norm: NormPoint::new(0.5, 0.50),
            zoom: ZoomSpec::FitWidth,
        };
        let a3 = ReadingAnchor {
            page: PageIndex(4),
            focal_norm: NormPoint::new(0.5, 0.10),
            zoom: ZoomSpec::FitWidth,
        };

        hist.push_before_jump(a1);
        hist.push_before_jump(a1); // duplicate ignored
        hist.push_before_jump(a2);
        assert!(hist.can_go_back());
        assert!(!hist.can_go_forward());

        let back1 = hist.go_back(a3).unwrap();
        assert_eq!(back1.page, PageIndex(2));
        assert!(hist.can_go_forward());

        let back2 = hist.go_back(back1).unwrap();
        assert_eq!(back2.page, PageIndex(0));
        assert!(!hist.can_go_back());

        let fwd1 = hist.go_forward(back2).unwrap();
        assert_eq!(fwd1.page, PageIndex(2));
    }

    #[test]
    fn find_active_portal_switches_by_reading_position() {
        let portals = vec![
            PortalLink {
                id: 1,
                src_page: PageIndex(0),
                src_focal_y: 0.25,
                dst: ReadingAnchor {
                    page: PageIndex(4),
                    focal_norm: NormPoint::new(0.5, 0.1),
                    zoom: ZoomSpec::FitWidth,
                },
                label: "Fig 1".to_string(),
            },
            PortalLink {
                id: 2,
                src_page: PageIndex(2),
                src_focal_y: 0.40,
                dst: ReadingAnchor {
                    page: PageIndex(6),
                    focal_norm: NormPoint::new(0.5, 0.2),
                    zoom: ZoomSpec::FitWidth,
                },
                label: "Theorem 3".to_string(),
            },
        ];

        assert_eq!(find_active_portal(&portals, PageIndex(0), 0.30).unwrap().id, 1);
        assert_eq!(find_active_portal(&portals, PageIndex(2), 0.55).unwrap().id, 2);
        assert_eq!(find_active_portal(&portals, PageIndex(3), 0.10).unwrap().id, 2);
        assert!(find_active_portal(&[], PageIndex(0), 0.0).is_none());
    }
}
