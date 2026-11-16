use gpui::{KeyBinding, Keystroke, Menu, MenuItem, OsAction, SystemMenuType, actions};

use crate::app_state::AppRegistry;

pub const APP_CONTEXT: &str = "PinleafApp";
pub const VIEWPORT_CONTEXT: &str = "PinleafViewport";
pub const NOTE_EDITOR_CONTEXT: &str = "PinleafNoteEditor";
pub const COMMAND_POPUP_CONTEXT: &str = "PinleafCommandPopup";
pub const SEARCH_BAR_CONTEXT: &str = "PinleafSearchBar";
pub const OVERLAY_POPUP_CONTEXT: &str = "PinleafOverlayPopup";
pub const MARK_PENDING_CONTEXT: &str = "PinleafMarkPending";

const APP_NO_INPUT: &str = "PinleafApp && !PinleafNoteEditor && !PinleafCommandPopup && !PinleafSearchBar && !PinleafOverlayPopup && !PinleafMarkPending";
const VIEWPORT_NO_INPUT: &str = "PinleafViewport && !PinleafNoteEditor && !PinleafCommandPopup && !PinleafSearchBar && !PinleafOverlayPopup && !PinleafMarkPending";

actions!(
    pinleaf,
    [
        NewWindow,
        OpenFile,
        OpenDemo,
        SavePdf,
        SavePdfAs,
        QuitApp,
        UndoAnnot,
        RedoAnnot,
        ToggleSidebar,
        ToggleCommandPopup,
        OpenRecentPopup,
        OpenGlobalHighlightsPopup,
        ToggleAutoReload,
        OpenSearch,
        NextSearchMatch,
        PrevSearchMatch,
        ToggleTocPopup,
        ToggleFreeze,
        FreezeAndSplitRight,
        FreezeAndSplitDown,
        FreezeAndNewWindow,
        DetachPaneToWindow,
        ClosePane,
        CopySelection,
        HighlightSelection,
        AnnotateWithNote,
        LookupScholar,
        ZoomIn,
        ZoomOut,
        ZoomFitWidth,
        ZoomFitContentWidth,
        CycleColorMode,
        ToggleReadingRuler,
        NextPage,
        PrevPage,
        JumpBack,
        JumpForward,
        ScrollLineDown,
        ScrollLineUp,
        ScrollHalfPageDown,
        ScrollHalfPageUp,
        FirstPage,
        LastPage,
        BeginSetMark,
        BeginJumpMark,
        ToggleAreaMode,
    ]
);

pub fn register_keybindings(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("secondary-n", NewWindow, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-o", OpenFile, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-s", SavePdf, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-shift-s", SavePdfAs, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-q", QuitApp, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-z", UndoAnnot, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-shift-z", RedoAnnot, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-shift-a", ToggleSidebar, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-f", OpenSearch, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-p", OpenRecentPopup, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-shift-f", OpenGlobalHighlightsPopup, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-g", NextSearchMatch, Some(APP_CONTEXT)),
        KeyBinding::new("secondary-shift-g", PrevSearchMatch, Some(APP_CONTEXT)),
        KeyBinding::new("n", NextSearchMatch, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("shift-n", PrevSearchMatch, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("secondary-t", ToggleTocPopup, Some(APP_CONTEXT)),
        KeyBinding::new("t", ToggleTocPopup, Some(APP_NO_INPUT)),
        KeyBinding::new("/", ToggleCommandPopup, Some(APP_NO_INPUT)),
        KeyBinding::new("?", ToggleCommandPopup, Some(APP_NO_INPUT)),
        KeyBinding::new("shift-/", ToggleCommandPopup, Some(APP_NO_INPUT)),
        KeyBinding::new("f", ToggleFreeze, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("secondary-d", FreezeAndSplitRight, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-shift-d", FreezeAndSplitDown, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-shift-n", FreezeAndNewWindow, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-alt-w", DetachPaneToWindow, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-w", ClosePane, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-c", CopySelection, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-h", HighlightSelection, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-m", AnnotateWithNote, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-shift-k", LookupScholar, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-=", ZoomIn, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-+", ZoomIn, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary--", ZoomOut, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("secondary-0", ZoomFitWidth, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("f10", ZoomFitContentWidth, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("shift-w", ZoomFitContentWidth, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("f8", CycleColorMode, Some(APP_CONTEXT)),
        KeyBinding::new("r", ToggleReadingRuler, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("pagedown", NextPage, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("pageup", PrevPage, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("secondary-[", JumpBack, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("ctrl-o", JumpBack, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("secondary-]", JumpForward, Some(VIEWPORT_CONTEXT)),
        KeyBinding::new("ctrl-i", JumpForward, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("j", ScrollLineDown, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("down", ScrollLineDown, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("k", ScrollLineUp, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("up", ScrollLineUp, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("d", ScrollHalfPageDown, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("ctrl-d", ScrollHalfPageDown, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("u", ScrollHalfPageUp, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("ctrl-u", ScrollHalfPageUp, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("g", FirstPage, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("home", FirstPage, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("shift-g", LastPage, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("end", LastPage, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("m", BeginSetMark, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("`", BeginJumpMark, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("'", BeginJumpMark, Some(VIEWPORT_NO_INPUT)),
        KeyBinding::new("a", ToggleAreaMode, Some(VIEWPORT_NO_INPUT)),
    ]);

    let custom_keys = cx.global::<AppRegistry>().user_config.keybindings.clone();
    let mut custom_bindings = Vec::new();
    for (action_name, key_spec) in custom_keys {
        if Keystroke::parse(&key_spec).is_err() {
            continue;
        }
        match action_name.as_str() {
            "toggle_freeze" => {
                custom_bindings.push(KeyBinding::new(&key_spec, ToggleFreeze, Some(VIEWPORT_NO_INPUT)));
            }
            "open_search" => {
                custom_bindings.push(KeyBinding::new(&key_spec, OpenSearch, Some(APP_NO_INPUT)));
            }
            "toggle_toc" => {
                custom_bindings.push(KeyBinding::new(&key_spec, ToggleTocPopup, Some(APP_NO_INPUT)));
            }
            "cycle_color_mode" => {
                custom_bindings.push(KeyBinding::new(&key_spec, CycleColorMode, Some(APP_NO_INPUT)));
            }
            "zoom_fit_content" => {
                custom_bindings.push(KeyBinding::new(
                    &key_spec,
                    ZoomFitContentWidth,
                    Some(VIEWPORT_NO_INPUT),
                ));
            }
            "toggle_ruler" => {
                custom_bindings.push(KeyBinding::new(
                    &key_spec,
                    ToggleReadingRuler,
                    Some(VIEWPORT_NO_INPUT),
                ));
            }
            "quick_open" => {
                custom_bindings.push(KeyBinding::new(&key_spec, OpenRecentPopup, Some(APP_NO_INPUT)));
            }
            "global_highlights" => {
                custom_bindings.push(KeyBinding::new(
                    &key_spec,
                    OpenGlobalHighlightsPopup,
                    Some(APP_NO_INPUT),
                ));
            }
            "lookup_scholar" => {
                custom_bindings.push(KeyBinding::new(&key_spec, LookupScholar, Some(VIEWPORT_NO_INPUT)));
            }
            "toggle_auto_reload" => {
                custom_bindings.push(KeyBinding::new(
                    &key_spec,
                    ToggleAutoReload,
                    Some(APP_NO_INPUT),
                ));
            }
            _ => {}
        }
    }
    if !custom_bindings.is_empty() {
        cx.bind_keys(custom_bindings);
    }
}

pub fn install_app_menus(cx: &mut gpui::App) {
    cx.set_menus([
        Menu::new("Pinleaf").items([
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Command Palette…", ToggleCommandPopup),
            MenuItem::separator(),
            MenuItem::action("Quit Pinleaf", QuitApp),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Window", NewWindow),
            MenuItem::action("Open PDF…", OpenFile),
            MenuItem::action("Quick Open Recent…", OpenRecentPopup),
            MenuItem::action("Open Built-in Demo", OpenDemo),
            MenuItem::separator(),
            MenuItem::action("Save PDF", SavePdf),
            MenuItem::action("Save PDF As…", SavePdfAs),
            MenuItem::action("Toggle Live Auto-Reload", ToggleAutoReload),
            MenuItem::separator(),
            MenuItem::action("Close Pane / Window", ClosePane),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo Annotation", UndoAnnot, OsAction::Undo),
            MenuItem::os_action("Redo Annotation", RedoAnnot, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::action("Copy Selection", CopySelection),
            MenuItem::action("Highlight Selection", HighlightSelection),
            MenuItem::action("Highlight with Note", AnnotateWithNote),
            MenuItem::action("Lookup in Google Scholar", LookupScholar),
        ]),
        Menu::new("View").items([
            MenuItem::action("Search in Document…", OpenSearch),
            MenuItem::action("Global Highlight Search…", OpenGlobalHighlightsPopup),
            MenuItem::action("Table of Contents…", ToggleTocPopup),
            MenuItem::action("Toggle Annotations Sidebar", ToggleSidebar),
            MenuItem::separator(),
            MenuItem::action("Freeze / Unfreeze Pane", ToggleFreeze),
            MenuItem::action("Freeze & Split Right", FreezeAndSplitRight),
            MenuItem::action("Freeze & Split Down", FreezeAndSplitDown),
            MenuItem::action("Freeze & Open New Window", FreezeAndNewWindow),
            MenuItem::action("Detach Pane to Window", DetachPaneToWindow),
            MenuItem::separator(),
            MenuItem::action("Zoom In", ZoomIn),
            MenuItem::action("Zoom Out", ZoomOut),
            MenuItem::action("Fit Page Width", ZoomFitWidth),
            MenuItem::action("Fit Content Margins", ZoomFitContentWidth),
            MenuItem::action("Cycle Color Mode", CycleColorMode),
            MenuItem::action("Toggle Reading Ruler", ToggleReadingRuler),
        ]),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{KeyBindingContextPredicate, KeyContext, Keystroke};

    #[test]
    fn command_popup_keystrokes_parse() {
        for raw in [
            "/",
            "?",
            "shift-/",
            "secondary-f",
            "secondary-g",
            "secondary-shift-g",
            "secondary-[",
            "secondary-]",
            "ctrl-o",
            "ctrl-i",
            "shift-g",
            "shift-n",
            "shift-w",
            "f8",
            "f10",
            "r",
            "`",
            "'",
            "secondary-n",
            "secondary-q",
        ] {
            assert!(Keystroke::parse(raw).is_ok(), "failed to parse {raw}");
        }
    }

    #[test]
    fn no_input_predicate_excludes_note_search_overlay_and_mark_contexts() {
        let app_pred = KeyBindingContextPredicate::parse(APP_NO_INPUT).unwrap();
        let viewport_pred = KeyBindingContextPredicate::parse(VIEWPORT_NO_INPUT).unwrap();

        let normal_stack = [
            KeyContext::parse(APP_CONTEXT).unwrap(),
            KeyContext::parse(VIEWPORT_CONTEXT).unwrap(),
        ];
        assert!(app_pred.depth_of(&normal_stack).is_some());
        assert!(viewport_pred.depth_of(&normal_stack).is_some());

        for blocking in [
            NOTE_EDITOR_CONTEXT,
            COMMAND_POPUP_CONTEXT,
            SEARCH_BAR_CONTEXT,
            OVERLAY_POPUP_CONTEXT,
            MARK_PENDING_CONTEXT,
        ] {
            let blocked_stack = [
                KeyContext::parse(APP_CONTEXT).unwrap(),
                KeyContext::parse(VIEWPORT_CONTEXT).unwrap(),
                KeyContext::parse(blocking).unwrap(),
            ];
            assert_eq!(app_pred.depth_of(&blocked_stack), None, "blocking={blocking}");
            assert_eq!(
                viewport_pred.depth_of(&blocked_stack),
                None,
                "blocking={blocking}"
            );
        }
    }
}
