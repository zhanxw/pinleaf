#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandId {
    ToggleFreeze,
    FreezeAndSplitRight,
    FreezeAndSplitDown,
    FreezeAndNewWindow,
    DetachPaneToWindow,
    ClosePane,
    HighlightSelection,
    AnnotateWithNote,
    UnderlineSelection,
    StrikeOutSelection,
    ToggleAreaMode,
    CopySelection,
    LookupScholar,
    ToggleSidebar,
    UndoAnnot,
    RedoAnnot,
    NextPage,
    PrevPage,
    ScrollScreenDown,
    ScrollScreenUp,
    JumpBack,
    JumpForward,
    FirstPage,
    LastPage,
    OpenSearch,
    NextSearchMatch,
    PrevSearchMatch,
    ToggleTocPopup,
    GoToPage,
    ToggleThumbnails,
    ToggleThumbnailGrid,
    BeginSetMark,
    BeginJumpMark,
    ZoomIn,
    ZoomOut,
    ZoomFitWidth,
    ZoomFitContentWidth,
    CycleColorMode,
    ToggleSmartDarkMode,
    ToggleReadingRuler,
    ToggleAutoScroll,
    TogglePresentationMode,
    LinkPortal,
    TogglePortalFollower,
    DeleteNearestPortal,
    BeginHintJump,
    BeginHintOverview,
    BeginHintSelect,
    BeginTypedHighlight,
    SynctexInverseJump,
    OpenExtensionsPopup,
    NewWindow,
    OpenFile,
    OpenDemo,
    OpenRecentPopup,
    OpenGlobalHighlightsPopup,
    ToggleAutoReload,
    SavePdf,
    SavePdfAs,
    ShowAbout,
    QuitApp,
    CloseCommandPopup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandEntry {
    pub id: CommandId,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub shortcut: &'static str,
    pub keywords: &'static str,
}

const fn platform_shortcut(mac: &'static str, other: &'static str) -> &'static str {
    if cfg!(target_os = "macos") {
        mac
    } else {
        other
    }
}

const COMMANDS: &[CommandEntry] = &[
    // Multi-View Freeze & Trail
    CommandEntry {
        id: CommandId::ToggleFreeze,
        category: "Multi-View Freeze & Trail",
        title: "Freeze / Unfreeze Pane",
        description: "Lock or unlock current pane's page, scroll, and zoom",
        shortcut: "F",
        keywords: "lock unlock pin anchor viewport",
    },
    CommandEntry {
        id: CommandId::FreezeAndSplitRight,
        category: "Multi-View Freeze & Trail",
        title: "Freeze & Split Right",
        description: "Freeze current pane at locked scale and open a new reading pane on the right",
        shortcut: platform_shortcut("⌘D", "Ctrl+D"),
        keywords: "branch horizontal side by side split right",
    },
    CommandEntry {
        id: CommandId::FreezeAndSplitDown,
        category: "Multi-View Freeze & Trail",
        title: "Freeze & Split Down",
        description: "Freeze current pane at locked scale and open a new reading pane below",
        shortcut: platform_shortcut("⇧⌘D", "Ctrl+Shift+D"),
        keywords: "branch vertical split down bottom",
    },
    CommandEntry {
        id: CommandId::FreezeAndNewWindow,
        category: "Multi-View Freeze & Trail",
        title: "Freeze & Open New Window",
        description: "Freeze current pane and branch a new OS window at the same anchor",
        shortcut: platform_shortcut("⇧⌘N", "Ctrl+Shift+N"),
        keywords: "branch window spawn separate",
    },
    CommandEntry {
        id: CommandId::DetachPaneToWindow,
        category: "Multi-View Freeze & Trail",
        title: "Detach Pane to Window",
        description: "Detach the active split pane into its own standalone window",
        shortcut: platform_shortcut("⌥⌘W", "Ctrl+Alt+W"),
        keywords: "pop out undock tear off move window",
    },
    CommandEntry {
        id: CommandId::ClosePane,
        category: "Multi-View Freeze & Trail",
        title: "Close Focused Pane",
        description: "Close the focused split pane (or window if last pane)",
        shortcut: platform_shortcut("⌘W", "Ctrl+W"),
        keywords: "remove quit dismiss split",
    },
    // Selection & Annotations
    CommandEntry {
        id: CommandId::HighlightSelection,
        category: "Selection & Annotations",
        title: "Highlight Selection",
        description: "Highlight selected text in yellow",
        shortcut: platform_shortcut("⌘H", "Ctrl+H"),
        keywords: "mark color yellow markup",
    },
    CommandEntry {
        id: CommandId::AnnotateWithNote,
        category: "Selection & Annotations",
        title: "Highlight & Add Note",
        description: "Highlight selected text and open an inline note editor",
        shortcut: platform_shortcut("⌘M", "Ctrl+M"),
        keywords: "comment remark memo sticky popup",
    },
    CommandEntry {
        id: CommandId::UnderlineSelection,
        category: "Selection & Annotations",
        title: "Underline Selection",
        description: "Underline selected text in blue",
        shortcut: "Select → U",
        keywords: "line emphasis markup blue",
    },
    CommandEntry {
        id: CommandId::StrikeOutSelection,
        category: "Selection & Annotations",
        title: "Strike Out Selection",
        description: "Strike out selected text in pink",
        shortcut: "Select → S",
        keywords: "strikethrough cross out delete markup",
    },
    CommandEntry {
        id: CommandId::ToggleAreaMode,
        category: "Selection & Annotations",
        title: "Toggle Area Box Mode",
        description: "Toggle crosshair marquee mode to annotate figures or regions",
        shortcut: platform_shortcut("A / ⌥Drag", "A / Alt+Drag"),
        keywords: "rectangle square box diagram figure image marquee",
    },
    CommandEntry {
        id: CommandId::CopySelection,
        category: "Selection & Annotations",
        title: "Copy Selected Text",
        description: "Copy currently selected PDF text to clipboard",
        shortcut: platform_shortcut("⌘C", "Ctrl+C"),
        keywords: "clipboard yank text",
    },
    CommandEntry {
        id: CommandId::LookupScholar,
        category: "Selection & Annotations",
        title: "Lookup Selection on Google Scholar / DOI",
        description: "Open doi.org or Google Scholar search for selected text",
        shortcut: platform_shortcut("⇧⌘K", "Ctrl+Shift+K"),
        keywords: "scholar doi citation paper lookup browser",
    },
    CommandEntry {
        id: CommandId::ToggleSidebar,
        category: "Selection & Annotations",
        title: "Toggle Notes Sidebar",
        description: "Show or hide the document-wide Notes & Annotations sidebar",
        shortcut: platform_shortcut("⇧⌘A", "Ctrl+Shift+A"),
        keywords: "panel list comments annotations drawer",
    },
    CommandEntry {
        id: CommandId::UndoAnnot,
        category: "Selection & Annotations",
        title: "Undo Annotation Change",
        description: "Undo the most recent annotation change",
        shortcut: platform_shortcut("⌘Z", "Ctrl+Z"),
        keywords: "revert history edit",
    },
    CommandEntry {
        id: CommandId::RedoAnnot,
        category: "Selection & Annotations",
        title: "Redo Annotation Change",
        description: "Redo the most recently undone annotation change",
        shortcut: platform_shortcut("⇧⌘Z", "Ctrl+Shift+Z"),
        keywords: "reapply history edit",
    },
    // Navigation & Search
    CommandEntry {
        id: CommandId::OpenSearch,
        category: "Navigation & Search",
        title: "Search Document Text",
        description: "Incremental & regex (re:) full-text search across all pages",
        shortcut: platform_shortcut("⌘F", "Ctrl+F"),
        keywords: "find search regex text",
    },
    CommandEntry {
        id: CommandId::NextSearchMatch,
        category: "Navigation & Search",
        title: "Next Search Match",
        description: "Jump to the next search result match",
        shortcut: platform_shortcut("n / ⌘G", "n / Ctrl+G"),
        keywords: "find next search match",
    },
    CommandEntry {
        id: CommandId::PrevSearchMatch,
        category: "Navigation & Search",
        title: "Previous Search Match",
        description: "Jump to the previous search result match",
        shortcut: platform_shortcut("N / ⇧⌘G", "N / Ctrl+Shift+G"),
        keywords: "find previous search match",
    },
    CommandEntry {
        id: CommandId::ToggleTocPopup,
        category: "Navigation & Search",
        title: "Table of Contents",
        description: "Browse & filter document outline or inferred section headings",
        shortcut: platform_shortcut("T / ⌘T", "T / Ctrl+T"),
        keywords: "toc outline chapters headings sections",
    },
    CommandEntry {
        id: CommandId::GoToPage,
        category: "Navigation & Search",
        title: "Go to Page...",
        description: "Jump directly to a specific 1-based page number (also type <num>G or /<num>)",
        shortcut: platform_shortcut("⌥⌘G / ^G", "Ctrl+Alt+G / Ctrl+G"),
        keywords: "goto jump page number navigate",
    },
    CommandEntry {
        id: CommandId::ToggleThumbnails,
        category: "Navigation & Search",
        title: "Toggle Page Thumbnails Strip",
        description: "Show or hide the scrollable page thumbnails sidebar strip",
        shortcut: platform_shortcut("⇧⌘T", "Ctrl+Shift+T"),
        keywords: "thumbnails preview strip sidebar pages",
    },
    CommandEntry {
        id: CommandId::ToggleThumbnailGrid,
        category: "Navigation & Search",
        title: "Thumbnail Grid Overview",
        description: "Open a 4-column visual contact sheet of all PDF pages",
        shortcut: platform_shortcut("⌥⌘T", "Ctrl+Alt+T"),
        keywords: "thumbnails grid contact sheet overview pages",
    },
    CommandEntry {
        id: CommandId::JumpBack,
        category: "Navigation & Search",
        title: "Jump Back in History",
        description: "Return to previous reading position before link/search/TOC jump",
        shortcut: platform_shortcut("⌘[ / ^O", "Ctrl+[ / Ctrl+O"),
        keywords: "history back return location",
    },
    CommandEntry {
        id: CommandId::JumpForward,
        category: "Navigation & Search",
        title: "Jump Forward in History",
        description: "Step forward to next position in jump history",
        shortcut: platform_shortcut("⌘] / ^I", "Ctrl+] / Ctrl+I"),
        keywords: "history forward location",
    },
    CommandEntry {
        id: CommandId::BeginSetMark,
        category: "Navigation & Search",
        title: "Set Named Mark",
        description: "Save current reading anchor to a single-key slot (a-z, 0-9)",
        shortcut: "m + key",
        keywords: "bookmark pin mark position",
    },
    CommandEntry {
        id: CommandId::BeginJumpMark,
        category: "Navigation & Search",
        title: "Jump to Named Mark",
        description: "Jump (or smart-route) to a previously saved mark slot",
        shortcut: "` or ' + key",
        keywords: "bookmark goto mark position",
    },
    CommandEntry {
        id: CommandId::NextPage,
        category: "Navigation & Search",
        title: "Next Page",
        description: "Step to the next page in an active pane",
        shortcut: "PgDn",
        keywords: "forward scroll page down",
    },
    CommandEntry {
        id: CommandId::PrevPage,
        category: "Navigation & Search",
        title: "Previous Page",
        description: "Step to the previous page in an active pane",
        shortcut: "PgUp",
        keywords: "back scroll page up",
    },
    CommandEntry {
        id: CommandId::ScrollScreenDown,
        category: "Navigation & Search",
        title: "Scroll Down One Screen",
        description: "Scroll down by one screenful with reading overlap (macOS Preview style)",
        shortcut: "Space",
        keywords: "spacebar scroll page down preview",
    },
    CommandEntry {
        id: CommandId::ScrollScreenUp,
        category: "Navigation & Search",
        title: "Scroll Up One Screen",
        description: "Scroll up by one screenful with reading overlap",
        shortcut: "Shift+Space",
        keywords: "spacebar scroll page up preview",
    },
    CommandEntry {
        id: CommandId::FirstPage,
        category: "Navigation & Search",
        title: "Go to First Page",
        description: "Jump to the beginning of the document",
        shortcut: "g / Home",
        keywords: "top beginning start page 1",
    },
    CommandEntry {
        id: CommandId::LastPage,
        category: "Navigation & Search",
        title: "Go to Last Page",
        description: "Jump to the end of the document",
        shortcut: "G / End",
        keywords: "bottom end last page",
    },
    CommandEntry {
        id: CommandId::ZoomIn,
        category: "Navigation & Search",
        title: "Zoom In",
        description: "Zoom in on the active reading pane",
        shortcut: platform_shortcut("⌘+", "Ctrl++"),
        keywords: "magnify larger scale",
    },
    CommandEntry {
        id: CommandId::ZoomOut,
        category: "Navigation & Search",
        title: "Zoom Out",
        description: "Zoom out on the active reading pane",
        shortcut: platform_shortcut("⌘-", "Ctrl+-"),
        keywords: "shrink smaller scale",
    },
    CommandEntry {
        id: CommandId::ZoomFitWidth,
        category: "Navigation & Search",
        title: "Fit Page to Width",
        description: "Fit PDF page width to the active pane",
        shortcut: platform_shortcut("⌘0", "Ctrl+0"),
        keywords: "reset scale zoom fit",
    },
    CommandEntry {
        id: CommandId::ZoomFitContentWidth,
        category: "Navigation & Search",
        title: "Fit to Content Width",
        description: "Crop left/right white margins so body text fills the pane",
        shortcut: "F10 / W",
        keywords: "smart fit crop margin width",
    },
    CommandEntry {
        id: CommandId::ToggleReadingRuler,
        category: "Navigation & Search",
        title: "Toggle Visual Reading Ruler",
        description: "Highlight the active reading line and step lines with j / k",
        shortcut: "R",
        keywords: "ruler marker focus line column dyslexia",
    },
    CommandEntry {
        id: CommandId::CycleColorMode,
        category: "Navigation & Search",
        title: "Cycle Reading Theme (Light / Dark / Sepia / Custom)",
        description: "Switch between Light paper, Dark night, warm Sepia, and Custom config theme",
        shortcut: "F8",
        keywords: "dark mode sepia night light custom theme invert color",
    },
    CommandEntry {
        id: CommandId::ToggleSmartDarkMode,
        category: "Navigation & Search",
        title: "Toggle Smart Dark Figure Preservation",
        description: "Keep raster figures and plots un-inverted inside Dark & Custom themes",
        shortcut: "⇧F8",
        keywords: "smart dark figures images plots preserve",
    },
    CommandEntry {
        id: CommandId::ToggleAutoScroll,
        category: "Navigation & Search",
        title: "Toggle Smooth Auto-Scroll",
        description: "Start or stop 60 FPS continuous vertical scrolling (Alt+= / Alt+- speed)",
        shortcut: "F6",
        keywords: "autoscroll teleprompter scroll smooth",
    },
    CommandEntry {
        id: CommandId::TogglePresentationMode,
        category: "Navigation & Search",
        title: "Toggle Presentation Mode",
        description: "Hide toolbars & fit whole slides in fullscreen",
        shortcut: "F5",
        keywords: "fullscreen slides presentation chrome free",
    },
    // Power Features (Portals, Hints, Categories, SyncTeX, Extensions)
    CommandEntry {
        id: CommandId::LinkPortal,
        category: "Portals, Hints & SyncTeX",
        title: "Link Scroll Portal",
        description: "Bind current reading position (or active Overview) to a helper target anchor",
        shortcut: "P",
        keywords: "portal link figure theorem helper sync scroll",
    },
    CommandEntry {
        id: CommandId::TogglePortalFollower,
        category: "Portals, Hints & SyncTeX",
        title: "Toggle Portal Follower Pane",
        description: "Make this pane (or spawn a split) auto-follow scroll-linked portals",
        shortcut: "⇧P",
        keywords: "portal follower helper split pane",
    },
    CommandEntry {
        id: CommandId::DeleteNearestPortal,
        category: "Portals, Hints & SyncTeX",
        title: "Delete Nearest Scroll Portal",
        description: "Remove the closest scroll-linked portal from the current position",
        shortcut: "⌥P",
        keywords: "portal delete remove unlink",
    },
    CommandEntry {
        id: CommandId::BeginHintJump,
        category: "Portals, Hints & SyncTeX",
        title: "Keyboard Hint Jump (Links & Smart Refs)",
        description: "Overlay 2-letter hints over visible links and academic refs to jump without mouse",
        shortcut: ";",
        keywords: "hints vimium keyboard jump link smart ref",
    },
    CommandEntry {
        id: CommandId::BeginHintOverview,
        category: "Portals, Hints & SyncTeX",
        title: "Keyboard Hint Overview Preview",
        description: "Overlay 2-letter hints to open a floating Overview card from the keyboard",
        shortcut: ":",
        keywords: "hints vimium overview preview popup keyboard",
    },
    CommandEntry {
        id: CommandId::BeginHintSelect,
        category: "Portals, Hints & SyncTeX",
        title: "Keyboard Hint Text Select",
        description: "Pick start & end words via 2-letter hints for mouse-free text selection",
        shortcut: "V",
        keywords: "hints visual select text copy highlight keyboard",
    },
    CommandEntry {
        id: CommandId::BeginTypedHighlight,
        category: "Portals, Hints & SyncTeX",
        title: "Highlight with Category Bucket (a-z)",
        description: "Highlight selection or re-tag inspected highlight into one of 26 typed buckets",
        shortcut: "H + a-z",
        keywords: "highlight category type color bucket 26",
    },
    CommandEntry {
        id: CommandId::SynctexInverseJump,
        category: "Portals, Hints & SyncTeX",
        title: "SyncTeX Jump to TeX Source",
        description: "Open the LaTeX source file and line corresponding to current reading position",
        shortcut: platform_shortcut("⇧⌘J", "Ctrl+Shift+J"),
        keywords: "synctex latex tex source editor inverse jump",
    },
    CommandEntry {
        id: CommandId::OpenExtensionsPopup,
        category: "Portals, Hints & SyncTeX",
        title: "Extensions & External Commands",
        description: "Run configured shell/Python extensions with selected quote, file path, and DOI",
        shortcut: platform_shortcut("⇧⌘E", "Ctrl+Shift+E"),
        keywords: "extensions scripts shell python command external",
    },
    // Document & App
    CommandEntry {
        id: CommandId::NewWindow,
        category: "Document & App",
        title: "New Pinleaf Window",
        description: "Open a new independent reader window",
        shortcut: platform_shortcut("⌘N", "Ctrl+N"),
        keywords: "new window reader workspace",
    },
    CommandEntry {
        id: CommandId::OpenFile,
        category: "Document & Help",
        title: "Open PDF File...",
        description: "Open a PDF document from disk",
        shortcut: platform_shortcut("⌘O", "Ctrl+O"),
        keywords: "load browse file picker",
    },
    CommandEntry {
        id: CommandId::OpenDemo,
        category: "Document & Help",
        title: "Open Built-in Demo Paper",
        description: "Load the built-in 5-page multi-view demo paper",
        shortcut: "Toolbar",
        keywords: "sample fixture demo",
    },
    CommandEntry {
        id: CommandId::OpenRecentPopup,
        category: "Document & Help",
        title: "Quick-Open Recent PDFs",
        description: "Fuzzy-search and reopen recently viewed PDF documents",
        shortcut: platform_shortcut("⌘P", "Ctrl+P"),
        keywords: "recent library quick open history files",
    },
    CommandEntry {
        id: CommandId::OpenGlobalHighlightsPopup,
        category: "Document & Help",
        title: "Search Highlights Across All PDFs",
        description: "Search quotes and notes across all previously opened documents",
        shortcut: platform_shortcut("⇧⌘F", "Ctrl+Shift+F"),
        keywords: "global highlights notes search library all papers",
    },
    CommandEntry {
        id: CommandId::ToggleAutoReload,
        category: "Document & Help",
        title: "Toggle Live PDF Auto-Reload",
        description: "Automatically reload PDF when recompiled on disk (LaTeX / Typst)",
        shortcut: "Palette",
        keywords: "watch reload latex typst synctex recompile",
    },
    CommandEntry {
        id: CommandId::SavePdf,
        category: "Document & Help",
        title: "Save PDF Annotations",
        description: "Save highlights and notes directly into the PDF /Annots dictionary",
        shortcut: platform_shortcut("⌘S", "Ctrl+S"),
        keywords: "write persist export disk",
    },
    CommandEntry {
        id: CommandId::SavePdfAs,
        category: "Document & Help",
        title: "Save PDF As...",
        description: "Save annotated PDF to a new file path",
        shortcut: platform_shortcut("⇧⌘S", "Ctrl+Shift+S"),
        keywords: "copy export save new file",
    },
    CommandEntry {
        id: CommandId::ShowAbout,
        category: "Document & Help",
        title: "About Pinleaf",
        description: "Show Pinleaf program version and build information",
        shortcut: "Pinleaf Menu",
        keywords: "about version build info release",
    },
    CommandEntry {
        id: CommandId::QuitApp,
        category: "Document & App",
        title: "Quit Pinleaf",
        description: "Exit Pinleaf cleanly",
        shortcut: platform_shortcut("⌘Q", "Ctrl+Q"),
        keywords: "quit exit close application",
    },
    CommandEntry {
        id: CommandId::CloseCommandPopup,
        category: "Document & Help",
        title: "Commands & Shortcuts Help",
        description: "Toggle this commands & shortcuts popup",
        shortcut: "/ or ?",
        keywords: "palette help keybindings hotkeys list",
    },
];

pub fn all_commands() -> &'static [CommandEntry] {
    COMMANDS
}

pub fn filter_commands(query: &str) -> Vec<&'static CommandEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return all_commands().iter().collect();
    }

    let tokens: Vec<String> = trimmed
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .collect();

    let mut matches: Vec<(bool, usize, &'static CommandEntry)> = all_commands()
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| {
            let title_lower = entry.title.to_ascii_lowercase();
            let category_lower = entry.category.to_ascii_lowercase();
            let desc_lower = entry.description.to_ascii_lowercase();
            let shortcut_lower = entry.shortcut.to_ascii_lowercase();
            let keywords_lower = entry.keywords.to_ascii_lowercase();

            let all_match = tokens.iter().all(|tok| {
                title_lower.contains(tok)
                    || category_lower.contains(tok)
                    || desc_lower.contains(tok)
                    || shortcut_lower.contains(tok)
                    || keywords_lower.contains(tok)
            });

            if !all_match {
                return None;
            }

            let all_in_title = tokens.iter().all(|tok| title_lower.contains(tok));
            Some((all_in_title, idx, entry))
        })
        .collect();

    // Prioritize entries where every query token matches the command title while
    // preserving catalog order within each tier.
    matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    matches.into_iter().map(|(_, _, entry)| entry).collect()
}

/// Parses direct page-jump inputs typed into the Command Palette (`14`, `:14`, `p14`, `p.14`, `page 14`, `goto 14`).
pub fn parse_page_jump_query(query: &str, page_count: usize) -> Option<u32> {
    if page_count == 0 {
        return None;
    }
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    let rest = if let Some(s) = lower.strip_prefix("page ") {
        s.trim()
    } else if let Some(s) = lower.strip_prefix("goto ") {
        s.trim()
    } else if let Some(s) = lower.strip_prefix("p.") {
        s.trim()
    } else if let Some(s) = lower.strip_prefix('p') {
        s.trim()
    } else if let Some(s) = lower.strip_prefix(':') {
        s.trim()
    } else {
        lower.as_str()
    };

    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parsed: u32 = rest.parse().ok()?;
    if parsed == 0 {
        return None;
    }
    Some(parsed.clamp(1, page_count as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_returns_all_commands_in_order() {
        let all = filter_commands("   ");
        assert_eq!(all.len(), all_commands().len());
        assert_eq!(all[0].id, CommandId::ToggleFreeze);
        assert_eq!(all.last().unwrap().id, CommandId::CloseCommandPopup);
    }

    #[test]
    fn multi_token_filtering_matches_expected_commands() {
        let freeze_win = filter_commands("freeze window");
        assert!(
            freeze_win
                .iter()
                .any(|c| c.id == CommandId::FreezeAndNewWindow)
        );
        assert!(!freeze_win.iter().any(|c| c.id == CommandId::OpenFile));

        let zoom = filter_commands("zoom");
        let zoom_ids: Vec<_> = zoom.iter().map(|c| c.id).collect();
        assert!(zoom_ids.contains(&CommandId::ZoomIn));
        assert!(zoom_ids.contains(&CommandId::ZoomOut));
        assert!(zoom_ids.contains(&CommandId::ZoomFitWidth));

        let note = filter_commands("note");
        assert!(note.iter().any(|c| c.id == CommandId::AnnotateWithNote));
        assert!(note.iter().any(|c| c.id == CommandId::ToggleSidebar));

        let save = filter_commands("save");
        assert!(save.iter().any(|c| c.id == CommandId::SavePdf));
        assert!(save.iter().any(|c| c.id == CommandId::SavePdfAs));
    }

    #[test]
    fn title_matches_rank_ahead_of_description_only_matches() {
        // "window" appears in titles of FreezeAndOpenNewWindow & DetachPaneToWindow,
        // and only in description of ClosePane ("Close Focused Pane").
        let results = filter_commands("window");
        let close_pos = results
            .iter()
            .position(|c| c.id == CommandId::ClosePane)
            .expect("ClosePane matches via description");
        let new_win_pos = results
            .iter()
            .position(|c| c.id == CommandId::FreezeAndNewWindow)
            .expect("FreezeAndNewWindow matches in title");
        let detach_pos = results
            .iter()
            .position(|c| c.id == CommandId::DetachPaneToWindow)
            .expect("DetachPaneToWindow matches in title");

        assert!(new_win_pos < close_pos);
        assert!(detach_pos < close_pos);
    }

    #[test]
    fn unmatched_query_returns_empty() {
        assert!(filter_commands("nonexistent_command_xyz").is_empty());
    }

    #[test]
    fn parse_page_jump_query_handles_digits_prefixes_and_bounds() {
        assert_eq!(parse_page_jump_query("14", 40), Some(14));
        assert_eq!(parse_page_jump_query("  :7  ", 40), Some(7));
        assert_eq!(parse_page_jump_query("p12", 40), Some(12));
        assert_eq!(parse_page_jump_query("p. 3", 40), Some(3));
        assert_eq!(parse_page_jump_query("page 9", 40), Some(9));
        assert_eq!(parse_page_jump_query("goto 99", 40), Some(40));
        assert_eq!(parse_page_jump_query("0", 40), None);
        assert_eq!(parse_page_jump_query("p0", 40), None);
        assert_eq!(parse_page_jump_query("freeze", 40), None);
        assert_eq!(parse_page_jump_query("5", 0), None);
    }
}
