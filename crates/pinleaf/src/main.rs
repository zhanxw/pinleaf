mod actions;
mod app_state;
mod command_popup;
mod external;
mod icon;
mod ipc;
mod library;
mod session;
mod viewport;
mod workspace;

use std::path::PathBuf;
use std::sync::Arc;
use std::task::Waker;

use gpui::{
    App, AppContext, BorrowAppContext, Bounds, PathPromptOptions, QuitMode, TitlebarOptions,
    WindowBounds, WindowHandle, WindowOptions, px, size,
};
use parking_lot::Mutex;
use pinleaf_core::{
    FreezeState, NormPoint, PageIndex, PortalLink, ReadingAnchor, ReadingTrailGraph, ZoomSpec,
    find_active_portal,
};
use pinleaf_engine::{SyncTexMap, extract_page_text, render_page_bgra};

use crate::actions::{
    NewWindow, OpenFile, QuitApp, ShowAbout, install_app_menus, register_keybindings,
};
use crate::app_state::AppRegistry;
use crate::ipc::{
    IpcInbox, IpcRequest, default_ipc_socket_path, send_ipc_request, start_ipc_listener,
};
use crate::session::{ColorMode, DocumentSession, apply_color_mode_bgra_with_rects};
use crate::viewport::generate_hint_labels;
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

fn absolutize_cli_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path
    }
}

pub(crate) fn parse_synctex_forward_spec(spec: &str) -> Option<(u32, u32, PathBuf)> {
    let trimmed = spec.trim();
    let (line_str, rest) = trimmed.split_once(':')?;
    let line: u32 = line_str.parse().ok()?;
    if line == 0 || rest.is_empty() {
        return None;
    }
    if let Some((col_str, file_rest)) = rest.split_once(':')
        && let Ok(col) = col_str.parse::<u32>()
        && !file_rest.is_empty()
    {
        return Some((line, col, PathBuf::from(file_rest)));
    }
    Some((line, 0, PathBuf::from(rest)))
}

pub(crate) fn extract_cli_pdf_paths(args: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut skip_count = 0usize;
    for arg in args.iter().skip(1) {
        if skip_count > 0 {
            skip_count -= 1;
            continue;
        }
        if arg == "--synctex-forward" {
            skip_count = 2;
            continue;
        }
        if arg == "--execute-command" || arg == "--export-icons" {
            break;
        }
        if matches!(
            arg.as_str(),
            "-NSDocumentRevisionsDebugMode"
                | "-ApplePersistenceIgnoreState"
                | "-AppleLanguages"
        ) {
            skip_count = 1;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        if !arg.is_empty() {
            paths.push(absolutize_cli_path(PathBuf::from(arg)));
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

fn open_session_in_window(
    mut session_model: DocumentSession,
    cx: &mut App,
) -> Option<WindowHandle<WindowWorkspace>> {
    cx.global::<AppRegistry>()
        .prepare_session_model(&mut session_model);
    let title = format!("Pinleaf — {}", session_model.title);
    let session = cx.new(|_| session_model);
    AppRegistry::register_and_start_session(&session, 0, cx);

    let bounds = Bounds::centered(None, size(px(1180.), px(860.)), cx);
    cx.open_window(
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
    )
    .ok()
}

pub(crate) fn open_demo_window(cx: &mut App) -> Option<WindowHandle<WindowWorkspace>> {
    let session_model = DocumentSession::open_demo().expect("demo PDF loads");
    open_session_in_window(session_model, cx)
}

pub(crate) fn open_pdf_in_new_window(
    path: PathBuf,
    cx: &mut App,
) -> Option<WindowHandle<WindowWorkspace>> {
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
    open_session_in_window(session_model, cx)
}

fn focus_or_open_pdf(path: PathBuf, cx: &mut App) -> Option<WindowHandle<WindowWorkspace>> {
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
            return win.downcast::<WindowWorkspace>();
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
            return Some(ws_handle);
        }
    }

    let handle = open_pdf_in_new_window(path, cx);
    cx.activate(true);
    handle
}

fn handle_external_open_paths(paths: Vec<PathBuf>, cx: &mut App) {
    for path in paths {
        let _ = focus_or_open_pdf(path, cx);
    }
}

fn handle_ipc_request(req: IpcRequest, cx: &mut App) {
    match req {
        IpcRequest::OpenPdf {
            path,
            page_one_based,
        } => {
            if let Some(ws_handle) = focus_or_open_pdf(path, cx)
                && let Some(page) = page_one_based
            {
                let page_arg = [page.to_string()];
                let _ = ws_handle.update(cx, |ws, window, cx| {
                    let _ = ws.execute_ipc_command("goto_page", &page_arg, window, cx);
                    window.activate();
                    window.refresh();
                });
                cx.activate(true);
            }
        }
        IpcRequest::SynctexForward {
            pdf_path,
            tex_path,
            line,
            column: _,
        } => {
            if let Some(ws_handle) = focus_or_open_pdf(pdf_path, cx) {
                let _ = ws_handle.update(cx, |ws, window, cx| {
                    ws.focus_synctex_forward(&tex_path, line, cx);
                    window.activate();
                    window.refresh();
                });
                cx.activate(true);
            }
        }
        IpcRequest::ExecuteCommand { command, args } => match command.as_str() {
            "ping" => {}
            "quit" | "quit_app" => cx.quit(),
            "new_window" => {
                open_demo_window(cx);
                cx.activate(true);
            }
            "open" => {
                if let Some(path_str) = args.first() {
                    let _ = focus_or_open_pdf(PathBuf::from(path_str), cx);
                }
            }
            other => {
                let target_win = cx
                    .active_window()
                    .or_else(|| cx.windows().into_iter().next());
                if let Some(win) = target_win
                    && let Some(ws_handle) = win.downcast::<WindowWorkspace>()
                {
                    let _ = ws_handle.update(cx, |ws, window, cx| {
                        let _ = ws.execute_ipc_command(other, &args, window, cx);
                        window.refresh();
                    });
                }
            }
        },
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("pinleaf {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "Pinleaf {} — Multi-view academic PDF reader\n\n\
USAGE:\n  \
  pinleaf [PDF_FILE...]\n  \
  pinleaf --synctex-forward <line[:col]:file.tex> <paper.pdf>\n  \
  pinleaf --execute-command <command> [args...]\n  \
  pinleaf --self-test\n  \
  pinleaf --version | --help",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
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

    if let Some(pos) = args.iter().position(|a| a == "--execute-command") {
        let Some(command) = args.get(pos + 1).cloned() else {
            eprintln!("--execute-command requires a command name");
            std::process::exit(1);
        };
        let cmd_args: Vec<String> = args[pos + 2..].to_vec();
        let sock = default_ipc_socket_path();
        match send_ipc_request(&sock, &IpcRequest::ExecuteCommand { command, args: cmd_args }) {
            Ok(resp) if resp.ok => {
                println!("{}", resp.message);
                return;
            }
            Ok(resp) => {
                eprintln!("{}", resp.message);
                std::process::exit(1);
            }
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        }
    }

    let mut initial_synctex: Option<(PathBuf, PathBuf, u32, u32)> = None;
    if let Some(pos) = args.iter().position(|a| a == "--synctex-forward") {
        let Some(spec) = args.get(pos + 1) else {
            eprintln!("--synctex-forward requires <line[:col]:file.tex> <paper.pdf>");
            std::process::exit(1);
        };
        let Some((line, column, raw_tex)) = parse_synctex_forward_spec(spec) else {
            eprintln!("invalid --synctex-forward spec: {spec} (expected line[:col]:file.tex)");
            std::process::exit(1);
        };
        let pdf_arg = args
            .get(pos + 2)
            .filter(|s| !s.starts_with('-'))
            .map(PathBuf::from)
            .or_else(|| extract_cli_pdf_paths(&args).into_iter().next());
        let Some(raw_pdf) = pdf_arg else {
            eprintln!("--synctex-forward requires a target PDF path");
            std::process::exit(1);
        };
        let tex_path = absolutize_cli_path(raw_tex);
        let pdf_path = absolutize_cli_path(raw_pdf);
        let sock = default_ipc_socket_path();
        if let Ok(resp) = send_ipc_request(
            &sock,
            &IpcRequest::SynctexForward {
                pdf_path: pdf_path.clone(),
                tex_path: tex_path.clone(),
                line,
                column,
            },
        ) {
            if resp.ok {
                println!("{}", resp.message);
                return;
            }
            eprintln!("{}", resp.message);
            std::process::exit(1);
        }
        initial_synctex = Some((pdf_path, tex_path, line, column));
    }

    let cli_paths = extract_cli_pdf_paths(&args);
    let url_inbox = UrlInbox::default();
    let ipc_inbox = IpcInbox::default();

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
                cx.on_action(|_: &ShowAbout, cx| {
                    if cx.windows().is_empty()
                        && let Some(ws) = open_demo_window(cx)
                    {
                        let _ = ws.update(cx, |ws, window, cx| {
                            ws.open_about_popup(window, cx);
                        });
                        cx.activate(true);
                    }
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

                if let Some((pdf_path, tex_path, line, _col)) = initial_synctex {
                    if let Some(ws_handle) = open_pdf_in_new_window(pdf_path, cx) {
                        let _ = ws_handle.update(cx, |ws, _window, cx| {
                            ws.focus_synctex_forward(&tex_path, line, cx);
                        });
                    }
                } else if startup_paths.is_empty() {
                    open_demo_window(cx);
                } else {
                    for path in startup_paths {
                        let _ = open_pdf_in_new_window(path, cx);
                    }
                }

                cx.activate(true);

                if let Err(err) = start_ipc_listener(default_ipc_socket_path(), ipc_inbox.clone()) {
                    eprintln!("warning: could not start Pinleaf IPC listener: {err}");
                }

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

                let ipc_rx = ipc_inbox.clone();
                cx.spawn(async move |cx| {
                    loop {
                        let batch = ipc_rx.next_batch().await;
                        cx.update(|cx| {
                            for req in batch {
                                handle_ipc_request(req, cx);
                            }
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

    // Verify scroll-linked portals, Vimium hint labels, SyncTeX, and smart custom color mode
    let portals = vec![PortalLink {
        id: 1,
        label: "Fig. 1".to_string(),
        src_page: PageIndex(0),
        src_focal_y: 0.40,
        dst: ReadingAnchor {
            page: PageIndex(2),
            focal_norm: NormPoint::new(0.5, 0.25),
            zoom: ZoomSpec::FitWidth,
        },
    }];
    let matched_portal = find_active_portal(&portals, PageIndex(0), 0.42)
        .expect("expected active portal near src_focal_y 0.40");
    assert_eq!(matched_portal.dst.page, PageIndex(2));

    let hint_labels = generate_hint_labels(40);
    assert_eq!(hint_labels.len(), 40);
    let unique_hints: std::collections::HashSet<&str> =
        hint_labels.iter().map(String::as_str).collect();
    assert_eq!(unique_hints.len(), 40);
    assert!(hint_labels.iter().all(|l| l.len() == 2));

    let scale = 65781.76_f64;
    let sp = |bp: f64| -> i64 { (bp * scale).round() as i64 };
    let sample_stx = format!(
        "SyncTeX Version:1\n\
Input:1:./main.tex\n\
Output:pdf\n\
Magnification:1000\n\
Unit:1\n\
X Offset:0\n\
Y Offset:0\n\
Content:\n\
{{1\n\
[1,1:{},{}:{},{},{}\n\
x1,42,3:{},{}\n\
]\n\
}}\n\
Postamble:\n",
        sp(72.0),
        sp(720.0),
        sp(450.0),
        sp(500.0),
        sp(600.0),
        sp(120.0),
        sp(220.0),
    );
    let stx_dir = PathBuf::from("/tmp/pinleaf-self-test-stx");
    let stx = SyncTexMap::parse_str(&sample_stx, &stx_dir).expect("parse sample synctex");
    let fwd = stx
        .forward_search(&stx_dir.join("main.tex"), 42)
        .expect("forward search resolves line 42");
    assert_eq!(fwd.page, PageIndex(0));
    let inv = stx
        .inverse_search(PageIndex(0), NormPoint::new(120.0 / 612.0, 220.0 / 792.0))
        .expect("inverse search resolves line 42");
    assert_eq!(inv.line, 42);

    // Light BGRA pixel [255, 255, 255, 255] inside preserve rect stays unchanged; outside recolors
    let mut px_buf = vec![255u8; 4 * 4 * 4];
    apply_color_mode_bgra_with_rects(
        &mut px_buf,
        4,
        4,
        ColorMode::Custom,
        ([30, 30, 46], [205, 214, 244]),
        &[[2, 2, 4, 4]],
    );
    // Pixel (0, 0) outside figure -> custom bg [46, 30, 30] in BGRA order
    assert_eq!(&px_buf[0..4], &[46, 30, 30, 255]);
    // Pixel (3, 3) inside figure preserve rect -> remains pure white
    let p33 = (3 * 4 + 3) * 4;
    assert_eq!(&px_buf[p33..p33 + 4], &[255, 255, 255, 255]);

    // Verify thumbnail scaling & Command Palette page-jump parsing
    let thumb_s = crate::session::thumbnail_scale_for_page(612.0, 792.0);
    assert!(thumb_s > 0.1 && thumb_s < 1.0);
    assert_eq!(crate::command_popup::parse_page_jump_query("14", 20), Some(14));
    assert_eq!(
        crate::command_popup::parse_page_jump_query("page 5", 5),
        Some(5)
    );
    assert_eq!(crate::command_popup::parse_page_jump_query("6", 5), Some(5));
    assert_eq!(crate::command_popup::parse_page_jump_query("0", 5), None);

    println!(
        "self-test ok: {} pages, rendered {}x{} BGRA, {} TOC entries, freeze trail, smart jump, portals, hints, synctex, thumbnails & custom smart theme verified",
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

    #[test]
    fn parse_synctex_forward_spec_and_filter_cli_args() {
        assert_eq!(
            parse_synctex_forward_spec("42:10:/Users/test/main.tex"),
            Some((42, 10, PathBuf::from("/Users/test/main.tex")))
        );
        assert_eq!(
            parse_synctex_forward_spec("17:sections/intro.tex"),
            Some((17, 0, PathBuf::from("sections/intro.tex")))
        );
        assert_eq!(parse_synctex_forward_spec("0:/main.tex"), None);
        assert_eq!(parse_synctex_forward_spec("notanumber:/main.tex"), None);

        let args = vec![
            "pinleaf".to_string(),
            "--synctex-forward".to_string(),
            "42:1:/tmp/main.tex".to_string(),
            "/tmp/paper.pdf".to_string(),
            "/tmp/extra.pdf".to_string(),
        ];
        assert_eq!(
            extract_cli_pdf_paths(&args),
            vec![PathBuf::from("/tmp/extra.pdf")]
        );
    }
}
