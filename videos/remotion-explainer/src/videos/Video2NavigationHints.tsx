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
    interpolate(
      frame,
      [Math.round(durationInFrames * 0.35), Math.round(durationInFrames * 0.78)],
      [0, fullQuery.length],
      { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' },
    ),
  );
  const query = fullQuery.slice(0, charCount);
  const allItems = [
    { category: 'Navigation & Search', title: 'Search Document Text', shortcut: '⌘F' },
    { category: 'Portals & Hints', title: 'Keyboard Hint Jump (Links & Refs)', shortcut: ';' },
    { category: 'Portals & Hints', title: 'Keyboard Hint Overview Preview', shortcut: ':' },
    { category: 'Portals & Hints', title: 'Keyboard Hint Text Select', shortcut: 'V' },
    { category: 'Appearance', title: 'Cycle Theme (Light/Dark/Sepia/Custom)', shortcut: 'F8' },
  ];
  const filtered = query
    ? allItems.filter(
        (it) => it.title.toLowerCase().includes(query) || it.category.toLowerCase().includes(query),
      )
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

  const cursorX = interpolate(frame, [8, overviewAt - 6], [22, 39], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });
  const cursorY = interpolate(frame, [8, overviewAt - 6], [42, 26], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <PinleafWindow
      keyPressChip={showOverview ? 'Right-Click "Figure 1" → Overview Card' : 'Hovering Unlinked Ref'}
      panes={[
        {
          label: 'Pane #1 • Heuristic Smart Jump (Unlinked Refs)',
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
              pageLabel: 'Page 3 • Smart Ref',
              snippet:
                'Branch switch latency: Scroll (68ms), Tabs (52ms), Split (38ms), Scroll-Linked Portal (20ms). Inspect targets right in place without scrolling.',
              xPercent: 48,
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
        { code: 'as', xPercent: 12, yPercent: 24, typedPrefix: typedFirstChar ? 'a' : undefined },
        { code: 'df', xPercent: 26, yPercent: 24, typedPrefix: typedFirstChar ? 'd' : undefined },
        { code: 'jk', xPercent: 35, yPercent: 24, active: true, typedPrefix: typedFirstChar ? 'j' : undefined },
        { code: 'kl', xPercent: 21, yPercent: 30, active: frame >= selectPhaseAt + 24 },
      ]
    : [
        { code: 'as', xPercent: 18, yPercent: 17, active: true, typedPrefix: typedFirstChar ? 'a' : undefined },
        { code: 'df', xPercent: 39, yPercent: 17 },
        { code: 'gh', xPercent: 12, yPercent: 34 },
        { code: 'jk', xPercent: 36, yPercent: 34 },
      ];

  return (
    <PinleafWindow
      keyPressChip={
        inSelectMode
          ? 'Pressed  V  →  2-Endpoint Hint Select (jk → kl)'
          : 'Pressed  ;  (Jump) /  :  (Overview)  →  Hints'
      }
      panes={[
        {
          label: inSelectMode
            ? 'Hint Select Mode • Pick Start & End Words'
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
          ? 'Selected range ready for ⌘C copy or H+category highlight'
          : 'HINT_MODE_CONTEXT isolates hint keystrokes cleanly',
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
        heightPx={778}
        keyPressChip="T: Auto-TOC  •  ⌘F: Regex  •  m/`: Marks"
        panes={[
          {
            label: 'Pane #1 • Auto-TOC, Regex Search & Marks',
            active: true,
            pageProps: {
              pageNumber: 3,
              focusSection: 'theorem',
              highlights: [
                {
                  topPx: 212,
                  leftPercent: 6,
                  widthPercent: 20,
                  heightPx: 24,
                  color: '#34d399',
                  label: 'Theorem 1',
                },
                {
                  topPx: 316,
                  leftPercent: 52,
                  widthPercent: 22,
                  heightPx: 24,
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

      <div style={{ display: 'flex', gap: 16, fontFamily: FONT_SANS }}>
        {[
          { k: 'm a  /  ` a', l: 'Set & Jump Named Mark' },
          { k: '⌘[  /  ⌘]', l: 'Jump History (^O / ^I)' },
          { k: 'j k d u g G', l: 'Vim Scroll Keys' },
        ].map((chip) => (
          <div
            key={chip.k}
            style={{
              padding: '9px 18px',
              borderRadius: 11,
              backgroundColor: 'rgba(15, 23, 42, 0.9)',
              border: '1.5px solid rgba(96, 165, 250, 0.45)',
              fontSize: 19,
              fontWeight: 700,
              color: '#f1f5f9',
              display: 'flex',
              gap: 10,
            }}
          >
            <span style={{ fontFamily: FONT_MONO, fontWeight: 800, color: '#60a5fa' }}>{chip.k}</span>
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
