# Pinleaf Feature Explainer Videos (Remotion)

Reproducible Remotion project for Pinleaf's five 1920x1080 (30 FPS, H.264 + AAC) introductory feature videos.

## Prerequisites

- Node.js 20+
- Python 3 + `ffmpeg` / `ffprobe` (for voiceover synthesis on macOS via `/usr/bin/say`)

## Regenerate Voiceover Audio & Frame Timings

```bash
python3 scripts/generate_voiceover.py
```

This synthesizes 20 narration tracks using macOS `say` (`Samantha`), converts them to MP3 under `public/audio/`, and writes exact composition durations into `src/generated/timings.json`.

## Install & Render Videos

```bash
npm install
npx remotion browser ensure
npx tsc --noEmit

npx remotion render src/index.ts FreezeAndPortals ../../docs/videos/01-freeze-and-portals.mp4 --codec=h264
npx remotion render src/index.ts NavigationSmartJumpHints ../../docs/videos/02-navigation-smart-jump-hints.mp4 --codec=h264
npx remotion render src/index.ts AnnotationsCategoriesLibrary ../../docs/videos/03-annotations-categories-library.mp4 --codec=h264
npx remotion render src/index.ts ThemesComfortPresentation ../../docs/videos/04-themes-comfort-presentation.mp4 --codec=h264
npx remotion render src/index.ts SynctexIpcExtensions ../../docs/videos/05-synctex-ipc-extensions.mp4 --codec=h264
```
