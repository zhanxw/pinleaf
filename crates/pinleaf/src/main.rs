mod actions;
mod app_state;
mod command_popup;
mod external;
mod icon;
mod library;
mod session;
mod viewport;
mod workspace;

use std::path::PathBuf;
use std::sync::Arc;
use std::task::Waker;

use gpui::{
    App, AppContext, BorrowAppContext, Bounds, PathPromptOptions, QuitMode, TitlebarOptions,
    WindowBounds, WindowOptions, px, size,
};
use parking_lot::Mutex;
use pinleaf_core::{FreezeState, PageIndex, ReadingAnchor, ReadingTrailGraph};
use pinleaf_engine::{extract_page_text, render_page_bgra};

use crate::actions::{NewWindow, OpenFile, QuitApp, install_app_menus, register_keybindings};
use crate::app_state::AppRegistry;
use crate::session::DocumentSession;
use crate::workspace::WindowWorkspace;

#[derive(Default)]
struct UrlInboxState {
    paths: Vec<PathBuf>,
    waker: Option<Waker>,
}

#[derive(Clone, Default)]
struct UrlInbox(Arc<Mutex<UrlInboxState>>);

impl UrlInbox {
    fn push(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let mut guard = self.0.lock();
        guard.paths.extend(paths);
        if let Some(waker) = guard.waker.take() {
            waker.wake();
        }
    }

    fn drain(&self) -> Vec<PathBuf> {
        let mut guard = self.0.lock();
        std::mem::take(&mut guard.paths)
    }

    async fn next_batch(&self) -> Vec<PathBuf> {
        std::future::poll_fn(|cx| {
            let mut guard = self.0.lock();
            if guard.paths.is_empty() {
                guard.waker = Some(cx.waker().clone());
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(std::mem::take(&mut guard.paths))
            }
        })
        .await
    }
}

pub(crate) fn extract_cli_pdf_paths(args: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut skip_next_value = false;
    for arg in args.iter().skip(1) {
        if skip_next_value {
            skip_next_value = false;
            continue;
        }
        if matches!(
            arg.as_str(),
            "-NSDocumentRevisionsDebugMode"
                | "-ApplePersistenceIgnoreState"
                | "-AppleLanguages"
        ) {
            skip_next_value = true;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        if !arg.is_empty() {
            paths.push(PathBuf::from(arg));
        }
    }
    paths
}

pub(crate) fn parse_open_url_or_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let prefix = trimmed.get(..7);
    if prefix.is_some_and(|p| p.eq_ignore_ascii_case("file://")) {
        let after_scheme = &trimmed[7..];
        let without_host = if let Some(rest) = after_scheme.strip_prefix("localhost/") {
            format!("/{rest}")
        } else {
            after_scheme.to_string()
        };
        let path_part = without_host
            .split(['?', '#'])
            .next()
            .unwrap_or_default();
        if path_part.is_empty() {
            return None;
        }
        let decoded = urlencoding::decode(path_part)
            .map(|cow| cow.into_owned())
            .unwrap_or_else(|_| path_part.to_string());
        if decoded.is_empty() {
            None
        } else {
            Some(PathBuf::from(decoded))
        }
    } else if trimmed.starts_with('/') {
        Some(PathBuf::from(trimmed))
    } else {
        None
    }
}

fn open_session_in_window(mut session_model: DocumentSession, cx: &mut App) {
    cx.global::<AppRegistry>()
        .prepare_session_model(&mut session_model);
    let title = format!("Pinleaf — {}", session_model.title);
    let session = cx.new(|_| session_model);
    AppRegistry::register_and_start_session(&session, 0, cx);

    let bounds = Bounds::centered(None, size(px(1180.), px(860.)), cx);
    let _ = cx.open_window(
        WindowOptions::default()
            .window_bounds(WindowBounds::Windowed(bounds))
            .titlebar(Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            })),
        move |window, cx| {
            cx.new(|cx| {
                WindowWorkspace::new_with_anchor(
                    session,
                    None,
                    ReadingAnchor::default(),
                    window,
                    cx,
                )
            })
        },
    );
}

pub(crate) fn open_demo_window(cx: &mut App) {
    let session_model = DocumentSession::open_demo().expect("demo PDF loads");
    open_session_in_window(session_model, cx);
}

pub(crate) fn open_pdf_in_new_window(path: PathBuf, cx: &mut App) {
    let session_model = match DocumentSession::open_path(path.clone()) {
        Ok(s) => s,
        Err(err) => {
            let mut fallback = DocumentSession::open_demo().expect("demo PDF loads");
            fallback.status_message = Some((
                format!("Could not open {}: {err}", path.display()),
                true,
            ));
            fallback
        }
    };
    open_session_in_window(session_model, cx);
}

fn handle_external_open_paths(paths: Vec<PathBuf>, cx: &mut App) {
    for path in paths {
        if let Some(existing_session) = cx.global::<AppRegistry>().lookup_session(&path) {
            let target_window = cx
                .global::<AppRegistry>()
                .viewports
                .values()
                .find_map(|entry| {
                    let vp = entry.viewport.upgrade()?;
                    if vp.read(cx).session == existing_session {
                        Some(entry.window)
                    } else {
                        None
                    }
                });
            if let Some(win) = target_window {
                let _ = win.update(cx, |_, window, _| {
                    window.activate();
                    window.refresh();
                });
                cx.activate(true);
                continue;
            }
        }

        let windows = cx.windows();
        if windows.len() == 1
            && let Some(ws_handle) = windows[0].downcast::<WindowWorkspace>()
        {
            let replace_demo = windows[0]
                .read::<WindowWorkspace, _, _>(cx, |ws_entity, cx| {
                    ws_entity.read(cx).is_untouched_demo(cx)
                })
                .unwrap_or(false);
            if replace_demo {
                let _ = ws_handle.update(cx, |ws, window, cx| {
                    ws.open_external_path(path.clone(), cx);
                    window.activate();
                    window.refresh();
                });
                cx.activate(true);
                continue;
            }
        }

        open_pdf_in_new_window(path, cx);
        cx.activate(true);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--self-test") {
        run_self_test().expect("self-test failed");
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--export-icons") {
        let out_dir = args
            .get(pos + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
        icon::export_icon_bundle(&out_dir).expect("export icons failed");
        println!("exported icons to {}", out_dir.display());
        return;
    }

    let cli_paths = extract_cli_pdf_paths(&args);
    let url_inbox = UrlInbox::default();

    let app = gpui_platform::application().with_quit_mode(QuitMode::Default);
    app.on_open_urls({
        let inbox = url_inbox.clone();
        move |urls| {
            let parsed: Vec<PathBuf> = urls
                .iter()
                .filter_map(|raw| parse_open_url_or_path(raw))
                .collect();
            if !parsed.is_empty() {
                inbox.push(parsed);
            }
        }
    });
    app.on_reopen(|cx| {
        cx.spawn(async move |cx| {
            cx.update(|cx| {
                if cx.windows().is_empty() {
                    open_demo_window(cx);
                    cx.activate(true);
                }
            });
        })
        .detach();
    });

    app.run(move |cx: &mut App| {
        // Defer window creation to the next main-thread runloop tick so
        // `gpui_ce_macos`'s `did_finish_launching` drops its platform mutex
        // before `MacPlatform::open_window` locks it.
        cx.spawn(async move |cx| {
            cx.update(|cx| {
                AppRegistry::init(cx);
                crate::icon::install_macos_dock_icon();
                register_keybindings(cx);
                install_app_menus(cx);

                cx.on_action(|_: &QuitApp, cx| {
                    cx.quit();
                });
                cx.on_action(|_: &NewWindow, cx| {
                    open_demo_window(cx);
                    cx.activate(true);
                });
                cx.on_action(|_: &OpenFile, cx| {
                    if !cx.windows().is_empty() {
                        return;
                    }
                    let rx = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: false,
                        prompt: Some("Select PDF Document".into()),
                    });
                    cx.spawn(async move |cx| {
                        let Ok(Ok(Some(paths))) = rx.await else {
                            return;
                        };
                        if let Some(path) = paths.into_iter().next() {
                            cx.update(|cx| {
                                open_pdf_in_new_window(path, cx);
                                cx.activate(true);
                            });
                        }
                    })
                    .detach();
                });

                cx.on_window_closed(|cx, closed_win_id| {
                    cx.update_global::<AppRegistry, _>(|reg, _| {
                        let to_remove: Vec<_> = reg
                            .viewports
                            .iter()
                            .filter(|(_, entry)| entry.window.window_id() == closed_win_id)
                            .map(|(&id, _)| id)
                            .collect();
                        for id in to_remove {
                            reg.unregister_viewport(id);
                        }
                    });
                })
                .detach();

                let mut startup_paths = cli_paths;
                startup_paths.extend(url_inbox.drain());

                if startup_paths.is_empty() {
                    open_demo_window(cx);
                } else {
                    for path in startup_paths {
                        open_pdf_in_new_window(path, cx);
                    }
                }

                cx.activate(true);

                let inbox_rx = url_inbox.clone();
                cx.spawn(async move |cx| {
                    loop {
                        let batch = inbox_rx.next_batch().await;
                        cx.update(|cx| {
                            handle_external_open_paths(batch, cx);
                        });
                    }
                })
                .detach();
            });
        })
        .detach();
    });
}

fn run_self_test() -> anyhow::Result<()> {
    let mut session = DocumentSession::open_demo()?;
    assert_eq!(session.page_count(), 5);
    assert_eq!(session.annotations.all_sorted().len(), 1);
    assert!(session.toc_entries.len() >= 4, "expected inferred Auto-TOC entries");

    let frame = render_page_bgra(&session.pdf_handle, PageIndex(0), 1.5)?;
    assert!(frame.width > 500 && frame.height > 700);

    let text = extract_page_text(&session.pdf_handle, PageIndex(1))?;
    assert!(text.full_text.contains("Theorem 1"));

    let layouts = session.ensure_all_layouts_now();
    assert!(
        !pinleaf_engine::search_layouts(&layouts, "Theorem 1", false).is_empty(),
        "expected full-text search to find Theorem 1"
    );
    let fig_byte = layouts[0]
        .full_text
        .find("Figure 1")
        .expect("Figure 1 ref on demo page 1");
    let fig_glyph = layouts[0]
        .glyphs
        .iter()
        .position(|g| g.byte_range.start <= fig_byte + 2 && fig_byte + 2 <= g.byte_range.end)
        .expect("Figure 1 glyph index");
    let fig_target = pinleaf_engine::resolve_smart_jump_at(PageIndex(0), fig_glyph, &layouts)
        .expect("smart jump resolves Figure 1");
    assert_eq!(fig_target.page, PageIndex(2));

    // Verify N-node freeze-and-branch trail graph mechanics
    let mut trail = ReadingTrailGraph::new();
    let v1 = trail.register_root(FreezeState::Active);
    trail.set_freeze_state(v1.id, FreezeState::Frozen);
    let v2 = trail.register_child(Some(v1.id), FreezeState::Active);
    trail.set_freeze_state(v2.id, FreezeState::Frozen);
    let v3 = trail.register_child(Some(v2.id), FreezeState::Active);
    assert_eq!(trail.find_active_descendant(v1.id), Some(v3.id));

    println!(
        "self-test ok: {} pages, rendered {}x{} BGRA, {} TOC entries, 3-level freeze trail & smart jump verified",
        session.page_count(),
        frame.width,
        frame.height,
        session.toc_entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_open_url_handles_file_urls_and_paths() {
        assert_eq!(
            parse_open_url_or_path("file:///Users/test/My%20Paper%20%231.pdf"),
            Some(PathBuf::from("/Users/test/My Paper #1.pdf"))
        );
        assert_eq!(
            parse_open_url_or_path("file://localhost/tmp/a.pdf"),
            Some(PathBuf::from("/tmp/a.pdf"))
        );
        assert_eq!(
            parse_open_url_or_path("file:///tmp/paper.pdf?foo=bar#page=2"),
            Some(PathBuf::from("/tmp/paper.pdf"))
        );
        assert_eq!(
            parse_open_url_or_path("/Users/test/direct.pdf"),
            Some(PathBuf::from("/Users/test/direct.pdf"))
        );
        assert_eq!(parse_open_url_or_path("https://example.com/paper.pdf"), None);
        assert_eq!(parse_open_url_or_path("   "), None);
    }

    #[test]
    fn extract_cli_pdf_paths_filters_macos_launch_flags() {
        let args = vec![
            "/Applications/Pinleaf.app/Contents/MacOS/pinleaf".to_string(),
            "-psn_0_12345".to_string(),
            "-NSDocumentRevisionsDebugMode".to_string(),
            "YES".to_string(),
            "-ApplePersistenceIgnoreState".to_string(),
            "YES".to_string(),
            "/Users/test/Downloads/Paper 1.pdf".to_string(),
        ];
        assert_eq!(
            extract_cli_pdf_paths(&args),
            vec![PathBuf::from("/Users/test/Downloads/Paper 1.pdf")]
        );
    }
}
