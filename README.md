<p align="center">
  <img src="assets/icon_256.png" width="112" height="112" alt="Pinleaf Icon" />
</p>

<h1 align="center">Pinleaf</h1>

<p align="center">
  <strong>Pure-Rust multi-view academic PDF reader with unlimited freeze-and-branch panes, Smart Jump, and direct ISO-32000 PDF annotations.</strong>
</p>

<p align="center">
  <a href="https://github.com/zhanxw/pinleaf/actions/workflows/ci.yml">
    <img src="https://github.com/zhanxw/pinleaf/actions/workflows/ci.yml/badge.svg" alt="CI" />
  </a>
  <a href="https://github.com/zhanxw/pinleaf/actions/workflows/release.yml">
    <img src="https://github.com/zhanxw/pinleaf/actions/workflows/release.yml/badge.svg" alt="Build & Release" />
  </a>
  <a href="https://github.com/zhanxw/pinleaf/releases">
    <img src="https://img.shields.io/github/v/release/zhanxw/pinleaf" alt="Latest Release" />
  </a>
</p>

---

## Features

- **Unlimited Freeze & Branch Multi-View (`#1 🔒 → #2 🔒 → #3 → …`):**
  Freeze the active reading pane at its exact page, scroll anchor, and zoom scale, then split right (`⌘D`), split down (`⇧⌘D`), or open a new OS window (`⇧⌘N`) to chase derivations, theorems, and citations without losing your place. Clicking internal links or Smart Jumps inside a frozen pane automatically routes the active descendant viewport.
- **Native ISO-32000 PDF `/Annots` Persistence:**
  Select text across columns, highlight (`⌘H`), add sticky-note highlights (`⌘M`), underline, strikeout, or draw area marquees (`A`) with full undo/redo (`⌘Z` / `⇧⌘Z`) and atomic incremental writes (`⌘S`) directly into the PDF file so Apple Preview and other readers see your annotations.
- **Academic Reading Ergonomics (Sioyek-inspired):**
  - **Heuristic Smart Jump:** Click unlinked references like `Figure 1`, `Theorem 2`, `Table 1`, `Eq. (3)`, or `[12]` to jump to their target — or **Right-Click** for a floating **Overview** preview without scrolling away.
  - **Embedded + Auto-TOC (`⌘T` / `T`):** Fuzzy-filter document outlines, or automatically infer headings from font hierarchy and section numbers when `/Outlines` is absent.
  - **Full-Text & Regex Search (`⌘F`), Quick-Open Recent (`⌘P`), & Global Highlight Search (`⇧⌘F`).**
  - **Reading Comfort:** Fit-to-content-width (`F10` / `W`), Light / Dark / Sepia themes (`F8`), movable Visual Reading Ruler (`R`), Vim-style navigation (`j`/`k`/`d`/`u`/`g`/`G`), jump history (`⌘[` / `⌘]`), location marks (`m` / `` ` ``), Google Scholar / DOI lookup (`⇧⌘K`), and live 800 ms PDF auto-reload for LaTeX/Typst workflows.
- **Pure Rust + Metal GPU UI:**
  Built on `gpui-ce`, `hayro`, `lopdf`, and `resvg`.

---

## Installation (macOS)

1. Download `Pinleaf-v*-macos-universal.dmg` (or `.app.zip`) from [GitHub Releases](https://github.com/zhanxw/pinleaf/releases).
2. Drag `Pinleaf.app` into `/Applications`.
3. Because release builds are ad-hoc signed, strip the Gatekeeper quarantine attribute on first launch:
   ```bash
   xattr -dr com.apple.quarantine /Applications/Pinleaf.app
   ```

### Build & Package from Source

```bash
# Run workspace unit tests and built-in headless self-test
cargo test --locked --workspace
cargo run --locked -p pinleaf -- --self-test

# Build and package Pinleaf.app + Pinleaf.dmg (add --universal for arm64+x86_64)
./scripts/package_macos.sh
```

Custom user defaults and keybindings live in `~/.config/pinleaf/config.json`:

```json
{
  "default_color_mode": "Sepia",
  "auto_reload": true,
  "keybindings": {
    "open_search": "secondary-/"
  }
}
```

---

## Essential Shortcuts

| Action | Shortcut |
| :--- | :--- |
| Command Palette | `/` or `?` |
| Freeze / Unfreeze Pane | `F` |
| Freeze & Split Right / Down | `⌘D` / `⇧⌘D` |
| Freeze & New Window / Detach | `⇧⌘N` / `⌘⌥W` |
| Table of Contents | `⌘T` or `T` |
| Search in Document / Next / Prev | `⌘F` / `n` / `N` |
| Quick-Open Recent / Global Highlights | `⌘P` / `⇧⌘F` |
| Highlight / Note / Lookup Scholar | `⌘H` / `⌘M` / `⇧⌘K` |
| Fit Width / Fit Content Margins | `⌘0` / `F10` or `W` |
| Light / Dark / Sepia Theme | `F8` |
| Reading Ruler / Area Mode | `R` / `A` |
| Set Mark / Jump to Mark | `m<char>` / `` `<char> `` |
| Jump Back / Forward | `⌘[` (`Ctrl+O`) / `⌘]` (`Ctrl+I`) |
| Save / Save As / Open / New Window | `⌘S` / `⇧⌘S` / `⌘O` / `⌘N` |

---

## Workspace Layout

- `crates/pinleaf-core` — Normalized coordinates, selection geometry, `AnnotationStore`, `ReadingTrailGraph`, and `JumpHistory`.
- `crates/pinleaf-engine` — `hayro` PDF rasterizer, 2-column glyph/line extractor, `/Link` destination resolver, Auto-TOC, search, and Smart Jump heuristics.
- `crates/pinleaf-annot` — ISO-32000 `/Annots` reader and atomic incremental PDF writer (`lopdf`).
- `crates/pinleaf` — Metal GPU GPUI application, split/window coordinator, macOS menu bar & URL handler, and SVG icon pipeline.
