# Pinleaf Feature Explainer Videos (Remotion)

Reproducible Remotion project for Pinleaf's five 1920x1080 (30 FPS, H.264 + AAC) introductory feature videos.

## Prerequisites

- Node.js 20+
- `uv` + `ffmpeg` / `ffprobe` (for neural voiceover synthesis via `edge-tts`)

## Regenerate Voiceover Audio & Frame Timings

```bash
uv run scripts/generate_voiceover.py
# Optional voice override:
# uv run scripts/generate_voiceover.py --voice en-US-AvaMultilingualNeural
```

This synthesizes 20 neural voiceover tracks using `edge-tts` (`en-US-AndrewMultilingualNeural` by default), loudness-normalizes them to `-16 LUFS` into `public/audio/`, and writes exact composition durations into `src/generated/timings.json`.

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
