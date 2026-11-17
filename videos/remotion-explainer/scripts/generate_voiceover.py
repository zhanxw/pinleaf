#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#   "edge-tts>=7.0.0",
# ]
# ///
"""Synthesize natural neural voiceover tracks using `edge-tts`, loudness-normalize
with `ffmpeg`, measure exact durations, and write `src/generated/timings.json`."""

import argparse
import asyncio
import hashlib
import json
import math
import subprocess
import tempfile
from pathlib import Path

import edge_tts

FPS = 30
TRANSITION_FRAMES = 12
TAIL_PAD_FRAMES = 6
DEFAULT_VOICE = "en-US-AndrewMultilingualNeural"
DEFAULT_RATE = "+2%"

ROOT = Path(__file__).resolve().parent.parent
AUDIO_DIR = ROOT / "public" / "audio"
OUT_JSON = ROOT / "src" / "generated" / "timings.json"
CACHE_JSON = ROOT / "src" / "generated" / "voiceover_cache.json"

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
                "narration": "Branch deeper with Split Down, new windows, or Detach Pane. Clicking Theorem 2 inside a frozen pane automatically routes your active descendant.",
            },
            {
                "id": "v1_s3",
                "sceneTitle": "Scroll-Linked Portals",
                "shortcutBadge": "P  •  ⇧P  •  ⌥P",
                "caption": "Scroll-Linked Portals bind any reading paragraph to a figure or equation with P. Toggle a Portal Follower pane with Shift P.",
                "narration": "Scroll-Linked Portals bind any reading paragraph directly to a figure or equation with P. Toggle a Portal Follower pane anytime with Shift P.",
            },
            {
                "id": "v1_s4",
                "sceneTitle": "Automatic Follower Sync",
                "shortcutBadge": "LibraryStore",
                "caption": "As you scroll down the paper, your follower pane updates automatically from Figure 1 to Equation 4 and persists per document.",
                "narration": "As you scroll down the paper, your follower pane switches automatically from Figure 1 to Equation 4, saved cleanly for each document.",
            },
        ],
    },
    {
        "id": "NavigationSmartJumpHints",
        "filename": "02-navigation-smart-jump-hints.mp4",
        "episode": "02 / 05",
        "title": "Smart Jump, Hints & Navigation",
        "subtitle": "Go-to-Page, Page Thumbnails, Smart Jump, Overview & Vimium Hints",
        "accent": "#60a5fa",
        "scenes": [
            {
                "id": "v2_s1",
                "sceneTitle": "Command Palette & Go to Page",
                "shortcutBadge": "/  •  ⌥⌘G  •  <num>G",
                "caption": "Open the Command Palette with Slash to filter actions or type a page number directly, or jump via Option Command G, the page pill, or Vim count G.",
                "narration": "Open the Command Palette with Slash to filter actions or type a page number directly, or jump anytime with Option Command G, the page pill, or number G.",
            },
            {
                "id": "v2_s2",
                "sceneTitle": "Smart Jump & Keyboard Overview",
                "shortcutBadge": ":  •  Enter  •  S  •  Esc",
                "caption": "Smart Jump resolves unlinked figures, theorems, and citations. Open an Overview card and press Enter to jump, S to split, or Escape to close.",
                "narration": "Smart Jump resolves unlinked figures, theorems, and citations. Open a floating Overview preview, then press Enter to jump, S to split right, or Escape to close.",
            },
            {
                "id": "v2_s3",
                "sceneTitle": "Page Thumbnails Strip & 4-Col Grid",
                "shortcutBadge": "⇧⌘T  •  ⌥⌘T  •  ⌘Click",
                "caption": "Toggle the virtualized Page Thumbnails strip with Shift Command T, or open the four-column Thumbnail Grid with Option Command T to jump or Command-Enter split.",
                "narration": "Browse visually with the Page Thumbnails strip on Shift Command T, or pop open the four-column Thumbnail Grid on Option Command T to jump or split.",
            },
            {
                "id": "v2_s4",
                "sceneTitle": "Vimium Hints, Auto-TOC & Space Scroll",
                "shortcutBadge": "; / V  •  T  •  ⌘F  •  Space",
                "caption": "Navigate hands-free with two-letter Vimium hints, Auto-TOC on T, regex search on Command F, named marks, and Preview-style Spacebar screen scrolling.",
                "narration": "Stay hands-free with two-letter hints, Auto-TOC on T, regex search on Command F, single-key marks, and smooth Mac Preview style Spacebar scrolling.",
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
                "narration": "Select text cleanly across two columns to highlight, add inline notes, underline, or frame figures. Command S saves standard P-D-F annotations atomically right inside the file.",
            },
            {
                "id": "v3_s2",
                "sceneTitle": "26 Typed Highlight Categories",
                "shortcutBadge": "H + a–z",
                "caption": "Organize thoughts into twenty-six typed highlight buckets using H plus a through z, persisted inside the PDF and customizable in config JSON.",
                "narration": "Organize ideas into twenty-six typed highlight buckets with H plus A through Z, stored inside your P-D-F and customizable right inside your config file.",
            },
            {
                "id": "v3_s3",
                "sceneTitle": "Notes Sidebar & Category Filters",
                "shortcutBadge": "⇧⌘A",
                "caption": "Toggle the Notes Sidebar with Shift Command A. Filter your highlights live by category badge to isolate Theorems, Methods, or Open Questions.",
                "narration": "Toggle the Notes Sidebar with Shift Command A. Click any category chip to filter live for Theorems, Methods, or Open Questions.",
            },
            {
                "id": "v3_s4",
                "sceneTitle": "Global Search & DOI Lookup",
                "shortcutBadge": "⌘P  •  ⇧⌘F  •  ⇧⌘K",
                "caption": "Switch papers instantly with Quick-Open Recent, search quotes across your entire library with Shift Command F, or look up DOIs on Google Scholar.",
                "narration": "Switch papers fast with Quick-Open Recent, search quotes across every document with Shift Command F, or look up any D-O-I on Google Scholar.",
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
                "narration": "Cycle cleanly between Light, Dark, Warm Sepia, and Custom themes using F8, with your custom hex palette configured in your settings.",
            },
            {
                "id": "v4_s2",
                "sceneTitle": "Smart Dark Figure Preservation",
                "shortcutBadge": "⇧F8",
                "caption": "Shift F8 toggles Smart Dark Figure Preservation. Pinleaf detects raster plots and diagrams so charts keep crisp natural colors inside dark mode.",
                "narration": "Shift F8 toggles Smart Dark Figure Preservation. Pinleaf automatically detects plots and diagrams so every chart keeps crisp natural colors in dark mode.",
            },
            {
                "id": "v4_s3",
                "sceneTitle": "Content Fit, Ruler & Auto-Scroll",
                "shortcutBadge": "F10 / W  •  R  •  F6",
                "caption": "Crop whitespace with W for Content Width, track dense lines using the Visual Reading Ruler on R, and toggle smooth auto-scroll with F6.",
                "narration": "Trim blank margins with W for Content Width, track dense paragraphs with the Reading Ruler on R, and cruise smoothly at sixty frames per second with F6.",
            },
            {
                "id": "v4_s4",
                "sceneTitle": "Fullscreen Presentation Mode",
                "shortcutBadge": "F5  •  Esc",
                "caption": "Need to present slides? Press F5 for chrome-free fullscreen Presentation Mode that fits whole pages cleanly, and hit Escape to return.",
                "narration": "Presenting talk slides? Hit F5 for chrome-free fullscreen Presentation Mode that scales each slide cleanly, then press Escape when you are done.",
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
                "narration": "Pinleaf watches your compiled document on disk and reloads edits within eight hundred milliseconds, keeping every scroll anchor, zoom level, and split pane locked.",
            },
            {
                "id": "v5_s2",
                "sceneTitle": "Pure-Rust Two-Way SyncTeX",
                "shortcutBadge": "--synctex-forward  •  ⇧⌘J",
                "caption": "Built-in pure Rust SyncTeX powers instant two-way jumping. Forward search highlights compiled lines in cyan, and Command Click jumps back to your editor.",
                "narration": "A built-in pure Rust Sync-Tech parser enables instant two-way jumping. Forward search highlights compiled text in cyan, and Command Click jumps right back to your editor.",
            },
            {
                "id": "v5_s3",
                "sceneTitle": "Unix Domain Socket CLI IPC",
                "shortcutBadge": "--execute-command",
                "caption": "Script live windows over a Unix socket with execute-command to jump pages, toggle thumbnails, split panes, or run searches from any terminal.",
                "narration": "Automate live windows over a Unix domain socket. Run execute-command from your terminal to jump pages, toggle thumbnails, or split panes.",
            },
            {
                "id": "v5_s4",
                "sceneTitle": "Extensions & About Pinleaf",
                "shortcutBadge": "⇧⌘E  •  Pinleaf Menu",
                "caption": "Run shell or Python extensions with Shift Command E, and click the top-left Pinleaf menu anytime to open About Pinleaf and copy your build version.",
                "narration": "Run shell and Python extensions with Shift Command E, and click the top-left Pinleaf menu anytime to inspect About Pinleaf and copy your version.",
            },
        ],
    },
]


async def synthesize_scene(
    sem: asyncio.Semaphore,
    tmp_dir: Path,
    scene: dict,
    voice: str,
    rate: str,
    cache: dict[str, str],
    only_set: set[str] | None,
    force: bool,
) -> dict:
    sid = scene["id"]
    raw_mp3 = tmp_dir / f"{sid}_raw.mp3"
    final_mp3 = AUDIO_DIR / f"{sid}.mp3"
    key_hash = hashlib.sha256(f"{voice}|{rate}|{scene['narration']}".encode()).hexdigest()

    if force or not final_mp3.exists():
        should_synth = True
    elif only_set is not None:
        should_synth = sid in only_set
    else:
        should_synth = cache.get(sid) != key_hash

    if should_synth:
        async with sem:
            await asyncio.sleep(1.5)
            last_err: Exception | None = None
            for attempt in range(1, 7):
                try:
                    comm = edge_tts.Communicate(scene["narration"], voice, rate=rate)
                    await comm.save(str(raw_mp3))
                    last_err = None
                    break
                except Exception as err:
                    last_err = err
                    print(f"retry {attempt}/6 for {sid}: {err}")
                    await asyncio.sleep(2.5 * attempt)
            if last_err is not None:
                raise last_err

        subprocess.run(
            [
                "/opt/homebrew/bin/ffmpeg",
                "-y",
                "-i",
                str(raw_mp3),
                "-af",
                "loudnorm=I=-16:TP=-1.5:LRA=11,adelay=180|180,apad=pad_dur=0.48",
                "-ar",
                "44100",
                "-ac",
                "2",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                str(final_mp3),
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        print(f"synthesized {sid} -> {final_mp3.name}")
    else:
        print(f"cached {sid} -> {final_mp3.name}")

    cache[sid] = key_hash

    dur_str = subprocess.check_output(
        [
            "/opt/homebrew/bin/ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            str(final_mp3),
        ],
        text=True,
    ).strip()
    dur_sec = float(dur_str)
    duration_in_frames = math.ceil(dur_sec * FPS) + TAIL_PAD_FRAMES

    return {
        "id": sid,
        "sceneTitle": scene["sceneTitle"],
        "shortcutBadge": scene["shortcutBadge"],
        "caption": scene["caption"],
        "audioFile": f"audio/{sid}.mp3",
        "audioSeconds": round(dur_sec, 3),
        "durationInFrames": duration_in_frames,
    }


async def run_all(
    voice: str,
    rate: str,
    only_set: set[str] | None,
    force: bool,
) -> None:
    AUDIO_DIR.mkdir(parents=True, exist_ok=True)
    OUT_JSON.parent.mkdir(parents=True, exist_ok=True)

    cache: dict[str, str] = {}
    if CACHE_JSON.exists():
        try:
            cache = json.loads(CACHE_JSON.read_text())
        except Exception:
            cache = {}

    manifest = {
        "fps": FPS,
        "transitionFrames": TRANSITION_FRAMES,
        "voice": voice,
        "videos": {},
    }

    sem = asyncio.Semaphore(1)
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp = Path(tmpdir)
        for video in VIDEOS:
            scene_records = []
            for scene in video["scenes"]:
                rec = await synthesize_scene(
                    sem, tmp, scene, voice, rate, cache, only_set, force
                )
                scene_records.append(rec)
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
                f"{video['id']} ({voice}): {len(scene_records)} scenes, "
                f"{total_frames} frames ({total_frames / FPS:.2f}s)"
            )

    OUT_JSON.write_text(json.dumps(manifest, indent=2) + "\n")
    CACHE_JSON.write_text(json.dumps(cache, indent=2, sort_keys=True) + "\n")
    print(f"Wrote {OUT_JSON} and {CACHE_JSON}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate neural voiceover MP3s for Pinleaf explainer videos.")
    parser.add_argument("--voice", default=DEFAULT_VOICE, help=f"edge-tts neural voice (default: {DEFAULT_VOICE})")
    parser.add_argument("--rate", default=DEFAULT_RATE, help=f"Speech rate offset (default: {DEFAULT_RATE})")
    parser.add_argument("--only", default="", help="Comma-separated scene IDs to force-resynthesize (reuses existing MP3s for others)")
    parser.add_argument("--force", action="store_true", help="Force re-synthesis of all scenes")
    args = parser.parse_args()
    only_set = (
        {item.strip() for item in args.only.split(",") if item.strip()}
        if args.only.strip()
        else None
    )
    asyncio.run(run_all(args.voice, args.rate, only_set, args.force))


if __name__ == "__main__":
    main()
