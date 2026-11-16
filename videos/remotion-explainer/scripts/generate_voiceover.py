#!/usr/bin/env python3
"""Synthesize voiceover tracks with macOS `say`, convert to MP3 via `ffmpeg`,
and write exact frame timings to `src/generated/timings.json`."""

import json
import math
import subprocess
import tempfile
from pathlib import Path

FPS = 30
TRANSITION_FRAMES = 12
TAIL_PAD_FRAMES = 6

ROOT = Path(__file__).resolve().parent.parent
AUDIO_DIR = ROOT / "public" / "audio"
OUT_JSON = ROOT / "src" / "generated" / "timings.json"

VIDEOS = [
    {
        "id": "FreezeAndPortals",
        "filename": "01-freeze-and-portals.mp4",
        "episode": "01 / 05",
        "title": "Freeze-and-Branch & Scroll Portals",
        "subtitle": "Pin source context, spawn multi-pane trails, and link figures dynamically",
        "accent": "#34d399",
        "scenes": [
            {
                "id": "v1_s1",
                "sceneTitle": "Freeze & Split Right",
                "shortcutBadge": "F  /  ⌘D",
                "caption": "Reading dense papers means jumping constantly. Pinleaf lets you freeze your active pane with F, or split right with Command D.",
                "narration": "Reading dense papers means jumping constantly. Pinleaf lets you freeze your active pane with F, or split right with Command D.",
            },
            {
                "id": "v1_s2",
                "sceneTitle": "Multi-Generation Reading Trail",
                "shortcutBadge": "⇧⌘D  •  ⇧⌘N  •  ⌥⌘W",
                "caption": "Branch deeper with Split Down, new windows, or Detach Pane. Clicking Theorem 2 in a frozen pane smart-routes the active descendant.",
                "narration": "Branch deeper with Split Down, new windows, or Detach Pane. Clicking Theorem 2 in a frozen pane smart routes the active descendant.",
            },
            {
                "id": "v1_s3",
                "sceneTitle": "Scroll-Linked Portals",
                "shortcutBadge": "P  •  ⇧P  •  ⌥P",
                "caption": "Scroll-Linked Portals bind any reading paragraph to a figure or equation with P. Toggle a Portal Follower pane with Shift P.",
                "narration": "Scroll Linked Portals bind any reading paragraph to a figure or equation with P. Toggle a Portal Follower pane with Shift P.",
            },
            {
                "id": "v1_s4",
                "sceneTitle": "Automatic Follower Sync",
                "shortcutBadge": "LibraryStore",
                "caption": "As you scroll down the paper, your follower pane updates automatically from Figure 1 to Equation 4 and persists per document.",
                "narration": "As you scroll down the paper, your follower pane updates automatically from Figure 1 to Equation 4 and persists per document.",
            },
        ],
    },
    {
        "id": "NavigationSmartJumpHints",
        "filename": "02-navigation-smart-jump-hints.mp4",
        "episode": "02 / 05",
        "title": "Smart Jump, Hints & Navigation",
        "subtitle": "Command Palette, unlinked ref resolution, Vimium hints, TOC & regex search",
        "accent": "#60a5fa",
        "scenes": [
            {
                "id": "v2_s1",
                "sceneTitle": "Instant Command Palette",
                "shortcutBadge": "/  or  ?",
                "caption": "Press Slash or Question Mark anytime to open the Command Palette. Type keywords to filter actions and inspect platform shortcuts instantly.",
                "narration": "Press Slash or Question Mark anytime to open the Command Palette. Type keywords to filter actions and inspect platform shortcuts instantly.",
            },
            {
                "id": "v2_s2",
                "sceneTitle": "Heuristic Smart Jump & Overview",
                "shortcutBadge": "Click  •  Right-Click",
                "caption": "Heuristic Smart Jump resolves unlinked labels like Figure 1, Theorem 1, or Citation 12. Right-click opens a floating Overview card.",
                "narration": "Heuristic Smart Jump resolves unlinked labels like Figure 1, Theorem 1, or Citation 12. Right click opens a floating Overview card.",
            },
            {
                "id": "v2_s3",
                "sceneTitle": "Vimium 2-Letter Hints",
                "shortcutBadge": ";  •  :  •  V",
                "caption": "Stay completely keyboard-driven with Vimium two-letter hints. Semicolon jumps links, Colon opens Overview previews, and V selects text between two words.",
                "narration": "Stay completely keyboard driven with Vimium two letter hints. Semicolon jumps links, Colon opens Overview previews, and V selects text between two words.",
            },
            {
                "id": "v2_s4",
                "sceneTitle": "Auto-TOC, Regex Search & Marks",
                "shortcutBadge": "T  •  ⌘F  •  m / `  •  ⌘[",
                "caption": "Explore with Auto-TOC on T, regex search via Command F, single-key marks with m and backtick, jump history, and Vim scrolling.",
                "narration": "Explore with Auto TOC on T, regex search via Command F, single key marks with m and backtick, jump history, and Vim scrolling.",
            },
        ],
    },
    {
        "id": "AnnotationsCategoriesLibrary",
        "filename": "03-annotations-categories-library.mp4",
        "episode": "03 / 05",
        "title": "PDF Annotations, 26 Categories & Library",
        "subtitle": "Standard ISO-32000 /Annots persistence, typed highlight buckets & cross-PDF search",
        "accent": "#fbbf24",
        "scenes": [
            {
                "id": "v3_s1",
                "sceneTitle": "Standard PDF /Annots Writing",
                "shortcutBadge": "⌘H  •  ⌘M  •  A  •  ⌘S",
                "caption": "Select text cleanly across two columns to highlight, attach inline notes, underline, or frame figures. Command S saves standard PDF annotations atomically.",
                "narration": "Select text cleanly across two columns to highlight, attach inline notes, underline, or frame figures. Command S saves standard PDF annotations atomically.",
            },
            {
                "id": "v3_s2",
                "sceneTitle": "26 Typed Highlight Categories",
                "shortcutBadge": "H + a–z",
                "caption": "Organize thoughts into twenty-six typed highlight buckets using H plus a through z, persisted inside the PDF and customizable in config JSON.",
                "narration": "Organize thoughts into twenty six typed highlight buckets using H plus a through z, persisted inside the PDF and customizable in config JSON.",
            },
            {
                "id": "v3_s3",
                "sceneTitle": "Notes Sidebar & Category Filters",
                "shortcutBadge": "⇧⌘A",
                "caption": "Toggle the Notes Sidebar with Shift Command A. Filter your highlights live by category badge to isolate Theorems, Methods, or Open Questions.",
                "narration": "Toggle the Notes Sidebar with Shift Command A. Filter your highlights live by category badge to isolate Theorems, Methods, or Open Questions.",
            },
            {
                "id": "v3_s4",
                "sceneTitle": "Global Search & DOI Lookup",
                "shortcutBadge": "⌘P  •  ⇧⌘F  •  ⇧⌘K",
                "caption": "Switch papers instantly with Quick-Open Recent, search quotes across your entire library with Shift Command F, or look up DOIs on Google Scholar.",
                "narration": "Switch papers instantly with Quick Open Recent, search quotes across your entire library with Shift Command F, or look up DOIs on Google Scholar.",
            },
        ],
    },
    {
        "id": "ThemesComfortPresentation",
        "filename": "04-themes-comfort-presentation.mp4",
        "episode": "04 / 05",
        "title": "Smart Dark Mode, Comfort & Presentation",
        "subtitle": "Figure-preserving dark & custom themes, content fit, ruler, auto-scroll & F5 slides",
        "accent": "#c084fc",
        "scenes": [
            {
                "id": "v4_s1",
                "sceneTitle": "4 Reading Themes + Custom Palette",
                "shortcutBadge": "F8",
                "caption": "Cycle through Light, Dark, Warm Sepia, and Custom palette modes with F8. Custom background and foreground hex colors live in your config file.",
                "narration": "Cycle through Light, Dark, Warm Sepia, and Custom palette modes with F8. Custom background and foreground hex colors live in your config file.",
            },
            {
                "id": "v4_s2",
                "sceneTitle": "Smart Dark Figure Preservation",
                "shortcutBadge": "⇧F8",
                "caption": "Shift F8 toggles Smart Dark Figure Preservation. Pinleaf detects raster plots and diagrams so charts keep crisp natural colors inside dark mode.",
                "narration": "Shift F8 toggles Smart Dark Figure Preservation. Pinleaf detects raster plots and diagrams so charts keep crisp natural colors inside dark mode.",
            },
            {
                "id": "v4_s3",
                "sceneTitle": "Content Fit, Ruler & Auto-Scroll",
                "shortcutBadge": "F10 / W  •  R  •  F6",
                "caption": "Crop whitespace with W for Content Width, track dense lines using the Visual Reading Ruler on R, and toggle smooth auto-scroll with F6.",
                "narration": "Crop whitespace with W for Content Width, track dense lines using the Visual Reading Ruler on R, and toggle smooth auto scroll with F6.",
            },
            {
                "id": "v4_s4",
                "sceneTitle": "Fullscreen Presentation Mode",
                "shortcutBadge": "F5  •  Esc",
                "caption": "Need to present slides? Press F5 for chrome-free fullscreen Presentation Mode that fits whole pages cleanly, and hit Escape to return.",
                "narration": "Need to present slides? Press F5 for chrome free fullscreen Presentation Mode that fits whole pages cleanly, and hit Escape to return.",
            },
        ],
    },
    {
        "id": "SynctexIpcExtensions",
        "filename": "05-synctex-ipc-extensions.mp4",
        "episode": "05 / 05",
        "title": "SyncTeX, CLI IPC & Extensions",
        "subtitle": "Live LaTeX/Typst auto-reload, two-way SyncTeX, Unix socket IPC & shell extensions",
        "accent": "#22d3ee",
        "scenes": [
            {
                "id": "v5_s1",
                "sceneTitle": "Live LaTeX / Typst Auto-Reload",
                "shortcutBadge": "800 ms Auto-Reload",
                "caption": "Pinleaf watches your compiled PDF on disk and reloads edits automatically while keeping every scroll anchor, zoom level, and split pane intact.",
                "narration": "Pinleaf watches your compiled PDF on disk and reloads edits automatically while keeping every scroll anchor, zoom level, and split pane intact.",
            },
            {
                "id": "v5_s2",
                "sceneTitle": "Pure-Rust Two-Way SyncTeX",
                "shortcutBadge": "--synctex-forward  •  ⇧⌘J",
                "caption": "Built-in pure Rust SyncTeX powers instant two-way jumping. Forward search highlights compiled lines in cyan, and Command Click jumps back to your editor.",
                "narration": "Built in pure Rust SyncTeX powers instant two way jumping. Forward search highlights compiled lines in cyan, and Command Click jumps back to your editor.",
            },
            {
                "id": "v5_s3",
                "sceneTitle": "Unix Domain Socket CLI IPC",
                "shortcutBadge": "--execute-command",
                "caption": "Script live windows over a private Unix domain socket. Run execute-command from any terminal to jump pages, split panes, or run searches.",
                "narration": "Script live windows over a private Unix domain socket. Run execute command from any terminal to jump pages, split panes, or run searches.",
            },
            {
                "id": "v5_s4",
                "sceneTitle": "Shell & Python Extensions",
                "shortcutBadge": "⇧⌘E",
                "caption": "Press Shift Command E or custom hotkeys to run shell or Python extensions, safely passing selected quotes, DOIs, and paths to external workflows.",
                "narration": "Press Shift Command E or custom hotkeys to run shell or Python extensions, safely passing selected quotes, DOIs, and paths to external workflows.",
            },
        ],
    },
]


def main() -> None:
    AUDIO_DIR.mkdir(parents=True, exist_ok=True)
    OUT_JSON.parent.mkdir(parents=True, exist_ok=True)

    manifest = {"fps": FPS, "transitionFrames": TRANSITION_FRAMES, "videos": {}}

    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        for video in VIDEOS:
            scene_records = []
            for scene in video["scenes"]:
                sid = scene["id"]
                aiff_path = tmp / f"{sid}.aiff"
                mp3_path = AUDIO_DIR / f"{sid}.mp3"

                subprocess.run(
                    ["/usr/bin/say", "-v", "Samantha", "-r", "186", "-o", str(aiff_path), scene["narration"]],
                    check=True,
                )
                subprocess.run(
                    [
                        "/opt/homebrew/bin/ffmpeg",
                        "-y",
                        "-i",
                        str(aiff_path),
                        "-af",
                        "adelay=200|200,apad=pad_dur=0.50",
                        "-ar",
                        "44100",
                        "-ac",
                        "2",
                        "-c:a",
                        "libmp3lame",
                        "-b:a",
                        "192k",
                        str(mp3_path),
                    ],
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                dur_str = subprocess.check_output(
                    [
                        "/opt/homebrew/bin/ffprobe",
                        "-v",
                        "error",
                        "-show_entries",
                        "format=duration",
                        "-of",
                        "default=noprint_wrappers=1:nokey=1",
                        str(mp3_path),
                    ],
                    text=True,
                ).strip()
                dur_sec = float(dur_str)
                duration_in_frames = math.ceil(dur_sec * FPS) + TAIL_PAD_FRAMES

                scene_records.append(
                    {
                        "id": sid,
                        "sceneTitle": scene["sceneTitle"],
                        "shortcutBadge": scene["shortcutBadge"],
                        "caption": scene["caption"],
                        "audioFile": f"audio/{sid}.mp3",
                        "audioSeconds": round(dur_sec, 3),
                        "durationInFrames": duration_in_frames,
                    }
                )

            total_frames = sum(s["durationInFrames"] for s in scene_records) - (
                (len(scene_records) - 1) * TRANSITION_FRAMES
            )
            manifest["videos"][video["id"]] = {
                "id": video["id"],
                "filename": video["filename"],
                "episode": video["episode"],
                "title": video["title"],
                "subtitle": video["subtitle"],
                "accent": video["accent"],
                "totalFrames": total_frames,
                "durationSeconds": round(total_frames / FPS, 2),
                "scenes": scene_records,
            }
            print(
                f"{video['id']}: {len(scene_records)} scenes, "
                f"{total_frames} frames ({total_frames / FPS:.2f}s)"
            )

    OUT_JSON.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Wrote {OUT_JSON}")


if __name__ == "__main__":
    main()
