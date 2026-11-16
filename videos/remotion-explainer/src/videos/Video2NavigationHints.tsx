import React from 'react';
import { interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';
import timingsJson from '../generated/timings.json';
import type { TimingsManifest } from '../types';
import { FONT_MONO, FONT_SANS } from '../components/StageFrame';
import { PinleafWindow } from '../components/PinleafWindow';
import { VideoSequenceShell } from '../components/VideoSequenceShell';

const manifest = timingsJson as TimingsManifest;
const VIDEO = manifest.videos.NavigationSmartJumpHints;

const Scene1CommandPalette: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const fullQuery = 'hint';
  const charCount = Math.floor(
    interpolate(frame, [Math.round(durationInFrames * 0.35), Math.round(durationInFrames * 0.78)], [0, fullQuery.length], {
      extrapolateLeft: 'clamp',
      extrapolateRight: 'clamp',
    }),
  );
  const query = fullQuery.slice(0, charCount);
  const allItems = [
    { category: 'Navigation & Search', title: 'Search Document Text', shortcut: '⌘F' },
    { category: 'Portals, Hints & SyncTeX', title: 'Keyboard Hint Jump (Links & Smart Refs)', shortcut: ';' },
    { category: 'Portals, Hints & SyncTeX', title: 'Keyboard Hint Overview Preview', shortcut: ':' },
    { category: 'Portals, Hints & SyncTeX', title: 'Keyboard Hint Text Select', shortcut: 'V' },
    { category: 'Navigation & Search', title: 'Cycle Reading Theme (Light / Dark / Sepia / Custom)', shortcut: 'F8' },
  ];
  const filtered = query
    ? allItems.filter((it) => it.title.toLowerCase().includes(query) || it.category.toLowerCase().includes(query))
    : allItems;

  return (
    <PinleafWindow
      keyPressChip="Pressed  /  or  ?  →  Command Palette"
      panes={[
        {
          label: 'Pane #1 • Active Reader',
          active: true,
          pageProps: { pageNumber: 1, focusSection: 'intro' },
        },
      ]}
      commandPalette={{
        query,
        selectedIdx: query.length > 0 ? 0 : 1,
        items: filtered,
      }}
    />
  );
};

const Scene2SmartJumpOverview: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();

  const overviewAt = Math.round(durationInFrames * 0.38);
  const showOverview = frame >= overviewAt;
  const overviewPop = spring({
    frame: Math.max(0, frame - overviewAt),
    fps,
    config: { damping: 18, stiffness: 200 },
  });

  const cursorX = interpolate(frame, [8, overviewAt - 6], [24, 38.5], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });
  const cursorY = interpolate(frame, [8, overviewAt - 6], [42, 29], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <PinleafWindow
      keyPressChip={showOverview ? 'Right-Click "Figure 1" → Floating Overview' : 'Hovering Unlinked Ref'}
      panes={[
        {
          label: 'Pane #1 • Heuristic Smart Jump (Unlinked PDF Refs)',
          active: true,
          pageProps: {
            pageNumber: 1,
            focusSection: 'intro',
            pulseFigureRef: true,
            pulseTheoremRef: frame >= 15 && frame < overviewAt,
            pulseCitationRef: showOverview,
          },
        },
      ]}
      overviewCard={
        showOverview
          ? {
              title: 'Figure 1 (Resolved Target)',
              pageLabel: 'Page 3 • Auto-Detected',
              snippet:
                'Empirical branch switch latency across 4 reading architectures: Scroll (68ms), Tabs (52ms), Split (38ms), Scroll-Linked Portal (20ms). Stay on Page 1 while inspecting target.',
              xPercent: 45,
              yPercent: 20,
              scale: interpolate(overviewPop, [0, 1], [0.88, 1]),
            }
          : undefined
      }
      cursor={{
        xPercent: cursorX,
        yPercent: cursorY,
        clicking: frame >= overviewAt - 3 && frame <= overviewAt + 9,
      }}
    />
  );
};

const Scene3VimiumHints: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const selectPhaseAt = Math.round(durationInFrames * 0.54);
  const inSelectMode = frame >= selectPhaseAt;
  const typedFirstChar =
    (!inSelectMode && frame >= Math.round(durationInFrames * 0.25)) ||
    (inSelectMode && frame >= selectPhaseAt + 14);

  const hints = inSelectMode
    ? [
        { code: 'as', xPercent: 11, yPercent: 25, typedPrefix: typedFirstChar ? 'a' : undefined },
        { code: 'df', xPercent: 26, yPercent: 31, typedPrefix: typedFirstChar ? 'd' : undefined },
        { code: 'jk', xPercent: 18, yPercent: 49, active: true, typedPrefix: typedFirstChar ? 'j' : undefined },
        { code: 'kl', xPercent: 37, yPercent: 54, active: frame >= selectPhaseAt + 24 },
      ]
    : [
        { code: 'as', xPercent: 19, yPercent: 27, active: true, typedPrefix: typedFirstChar ? 'a' : undefined },
        { code: 'df', xPercent: 36, yPercent: 27 },
        { code: 'gh', xPercent: 22, yPercent: 60 },
        { code: 'jk', xPercent: 72, yPercent: 63 },
      ];

  return (
    <PinleafWindow
      keyPressChip={
        inSelectMode
          ? 'Pressed  V  →  2-Endpoint Hint Text Select (jk → kl)'
          : 'Pressed  ;  (Jump)  or  :  (Overview)  →  Home-Row Hints'
      }
      panes={[
        {
          label: inSelectMode
            ? 'Hint Select Mode • Pick Start & End Word Hints'
            : 'Hint Jump Mode • 2-Letter Home-Row Badges',
          active: true,
          pageProps: {
            pageNumber: 1,
            focusSection: 'intro',
            hints,
            activeSelectionText: inSelectMode && frame >= selectPhaseAt + 22,
          },
        },
      ]}
      statusBanner={{
        text: inSelectMode
          ? 'Selected range ready for ⌘C copy or H+category highlight — zero mouse needed'
          : 'HINT_MODE_CONTEXT blocks single-key shortcuts cleanly during hint entry',
        tone: 'ok',
      }}
    />
  );
};

const Scene4TocSearchMarks: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const fullSearch = String.raw`re:Theorem\s+\d+`;
  const typedLen = Math.floor(
    interpolate(frame, [10, Math.round(durationInFrames * 0.45)], [0, fullSearch.length], {
      extrapolateLeft: 'clamp',
      extrapolateRight: 'clamp',
    }),
  );

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14, alignItems: 'center' }}>
      <PinleafWindow
        heightPx={675}
        keyPressChip="T: Auto-TOC  •  ⌘F: Regex Search  •  m/`: Marks"
        panes={[
          {
            label: 'Pane #1 • TOC, Search & Marks',
            active: true,
            pageProps: {
              pageNumber: 3,
              focusSection: 'theorem',
              highlights: [
                {
                  topPx: 215,
                  leftPercent: 8,
                  widthPercent: 18,
                  heightPx: 20,
                  color: '#34d399',
                  label: 'Theorem 1',
                },
                {
                  topPx: 295,
                  leftPercent: 52,
                  widthPercent: 18,
                  heightPx: 20,
                  color: '#60a5fa',
                  label: 'Theorem 2',
                },
              ],
            },
          },
        ]}
        searchBar={{
          query: fullSearch.slice(0, typedLen),
          matchLabel: 'Match 2 / 4 (n / N)',
        }}
        tocOverlay={{
          query: 'Theorem',
          items: [
            { level: 0, title: '1. Introduction', page: 1, inferred: true },
            { level: 1, title: '3. Theoretical Bounds & Proofs', page: 3, inferred: true, selected: true },
            { level: 1, title: '4. Empirical Benchmarks', page: 5, inferred: true },
            { level: 0, title: '5. Variational Formulation', page: 7, inferred: true },
          ],
        }}
      />

      <div style={{ display: 'flex', gap: 14, fontFamily: FONT_SANS }}>
        {[
          { k: 'm a  /  ` a', l: 'Set & Jump Named Mark' },
          { k: '⌘[  /  ⌘]', l: 'Jump Back / Forward (^O / ^I)' },
          { k: 'j k d u g G', l: 'Vim Scroll Keys' },
        ].map((chip) => (
          <div
            key={chip.k}
            style={{
              padding: '7px 14px',
              borderRadius: 10,
              backgroundColor: 'rgba(15, 23, 42, 0.85)',
              border: '1px solid rgba(96, 165, 250, 0.35)',
              fontSize: 14,
              color: '#e2e8f0',
              display: 'flex',
              gap: 8,
            }}
          >
            <span style={{ fontFamily: FONT_MONO, fontWeight: 700, color: '#60a5fa' }}>{chip.k}</span>
            <span>{chip.l}</span>
          </div>
        ))}
      </div>
    </div>
  );
};

export const Video2NavigationHints: React.FC = () => {
  return (
    <VideoSequenceShell
      video={VIDEO}
      transitionFrames={manifest.transitionFrames}
      renderScene={(idx) => {
        switch (idx) {
          case 0:
            return <Scene1CommandPalette />;
          case 1:
            return <Scene2SmartJumpOverview />;
          case 2:
            return <Scene3VimiumHints />;
          case 3:
          default:
            return <Scene4TocSearchMarks />;
        }
      }}
    />
  );
};
