import React from 'react';
import { interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';
import timingsJson from '../generated/timings.json';
import type { TimingsManifest } from '../types';
import { FONT_MONO, FONT_SANS } from '../components/StageFrame';
import { PinleafWindow } from '../components/PinleafWindow';
import { VideoSequenceShell } from '../components/VideoSequenceShell';

const manifest = timingsJson as TimingsManifest;
const VIDEO = manifest.videos.NavigationSmartJumpHints;

const Scene1CommandPaletteAndGoToPage: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const modalAt = Math.round(durationInFrames * 0.5);
  const countPrefixAt = Math.round(durationInFrames * 0.78);
  const typedPalette = frame >= Math.round(durationInFrames * 0.22);
  const showGotoModal = frame >= modalAt && frame < countPrefixAt;
  const showCountPrefix = frame >= countPrefixAt;

  const items = typedPalette
    ? [
        { category: 'Navigation', title: 'Jump to Page 4', shortcut: 'Enter' },
        { category: 'Navigation', title: 'Go to Page...', shortcut: '⌥⌘G / ^G' },
        { category: 'Navigation', title: 'Toggle Page Thumbnails Strip', shortcut: '⇧⌘T' },
        { category: 'Navigation', title: 'Open Thumbnail Grid Overview', shortcut: '⌥⌘T' },
      ]
    : [
        { category: 'Navigation & Search', title: 'Search Document Text', shortcut: '⌘F' },
        { category: 'Navigation', title: 'Go to Page...', shortcut: '⌥⌘G / ^G' },
        { category: 'Portals & Hints', title: 'Keyboard Hint Jump (Links & Refs)', shortcut: ';' },
        { category: 'Portals & Hints', title: 'Keyboard Hint Overview Preview', shortcut: ':' },
      ];

  return (
    <PinleafWindow
      keyPressChip={
        showCountPrefix
          ? 'Typed  4G  ->  Vim Count Jump to Page 4'
          : showGotoModal
          ? 'Pressed  ⌥⌘G / Click p.1/8  ->  Go to Page'
          : typedPalette
          ? 'Typed  /4  ->  Direct Page Jump Match'
          : 'Pressed  /  ->  Command Palette'
      }
      panes={[
        {
          label: 'Active Reader',
          active: true,
          pagePrefix: showCountPrefix ? '4' : undefined,
          pageProps: {
            pageNumber: showCountPrefix ? 4 : 1,
            focusSection: showCountPrefix ? 'figure' : 'intro',
          },
        },
      ]}
      commandPalette={
        frame < modalAt
          ? {
              query: typedPalette ? '4' : '',
              selectedIdx: typedPalette ? 0 : 1,
              items,
            }
          : undefined
      }
      gotoPageModal={
        showGotoModal
          ? {
              query: frame >= modalAt + 18 ? '4' : '',
              currentPage: 1,
              pageCount: 8,
            }
          : undefined
      }
      statusBanner={
        showCountPrefix
          ? { text: 'Jumped directly to p.4/8 via Go-to-Page', tone: 'ok' }
          : undefined
      }
    />
  );
};

const Scene2SmartJumpOverview: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();

  const overviewAt = Math.round(durationInFrames * 0.28);
  const splitHoverAt = Math.round(durationInFrames * 0.68);
  const showOverview = frame >= overviewAt;
  const splitSelected = frame >= splitHoverAt;
  const overviewPop = spring({
    frame: Math.max(0, frame - overviewAt),
    fps,
    config: { damping: 18, stiffness: 200 },
  });

  const cursorX = interpolate(frame, [6, overviewAt - 6], [22, 39], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });
  const cursorY = interpolate(frame, [6, overviewAt - 6], [42, 26], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <PinleafWindow
      keyPressChip={
        splitSelected
          ? 'Overview Keys:  Enter Jump  •  S Split  •  Esc Close'
          : showOverview
          ? 'Right-Click /  :  ->  Overview Card Opened'
          : 'Hovering Unlinked Ref ("Figure 1")'
      }
      panes={[
        {
          label: 'Heuristic Smart Jump (Unlinked Refs)',
          active: true,
          pageProps: {
            pageNumber: 1,
            focusSection: 'intro',
            pulseFigureRef: true,
            pulseTheoremRef: frame >= 12 && frame < overviewAt,
            pulseCitationRef: showOverview,
          },
        },
      ]}
      overviewCard={
        showOverview
          ? {
              title: 'Figure 1',
              pageLabel: 'p.3',
              snippet:
                'Branch switch latency: Scroll (68ms), Tabs (52ms), Split (38ms), Scroll-Linked Portal (20ms). Press Enter to jump, S to split right, or Esc to close.',
              xPercent: 44,
              yPercent: 18,
              scale: interpolate(overviewPop, [0, 1], [0.88, 1]),
              activeBtn: splitSelected ? 'split' : 'jump',
            }
          : undefined
      }
      cursor={{
        xPercent: cursorX,
        yPercent: cursorY,
        clicking: frame >= overviewAt - 3 && frame <= overviewAt + 8,
      }}
    />
  );
};

const Scene3ThumbnailsStripAndGrid: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const gridAt = Math.round(durationInFrames * 0.5);
  const showGrid = frame >= gridAt;
  const activePage = frame < Math.round(durationInFrames * 0.25) ? 1 : 3;
  const gridSelected = frame < Math.round(durationInFrames * 0.75) ? 3 : 4;

  const stripPages = [
    { page: 1, lockedTrail: 1, notes: 2 },
    { page: 2, notes: 1 },
    { page: 3 },
    { page: 4, portals: 1 },
    { page: 5 },
  ];

  const gridPages = [
    { page: 1, section: 'Abstract', lockedTrail: 1, notes: 2 },
    { page: 2, section: 'Methods', notes: 1 },
    { page: 3, section: 'Theorems' },
    { page: 4, section: 'Figure 1', notes: 1 },
    { page: 5, section: 'Benchmarks' },
    { page: 6, section: 'SyncTeX' },
    { page: 7, section: 'Discussion' },
    { page: 8, section: 'References' },
  ];

  return (
    <PinleafWindow
      keyPressChip={
        showGrid
          ? 'Pressed  ⌥⌘T  ->  4-Col Thumbnail Grid (Enter / ⌘Enter)'
          : 'Pressed  ⇧⌘T  / Pages ->  Thumbnail Strip (⌘+Click Splits)'
      }
      thumbnailsStrip={
        !showGrid
          ? {
              activePage,
              pages: stripPages,
            }
          : undefined
      }
      panes={[
        {
          label: 'Anchored Context',
          frozen: true,
          pageProps: { pageNumber: 1, focusSection: 'intro' },
        },
        {
          label: 'Active Branch',
          active: true,
          pageProps: {
            pageNumber: showGrid ? gridSelected : activePage,
            focusSection: activePage === 1 && !showGrid ? 'intro' : 'figure',
          },
        },
      ]}
      thumbnailGridModal={
        showGrid
          ? {
              selectedPage: gridSelected,
              pages: gridPages,
            }
          : undefined
      }
      statusBanner={{
        text: showGrid
          ? 'hjkl / Arrows move • Cmd+Enter freeze & split right'
          : '256-slot thumbnail LRU cache • max 8 parallel renders',
        tone: 'ok',
      }}
    />
  );
};

const Scene4HintsTocSearchSpace: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const phase2At = Math.round(durationInFrames * 0.46);
  const inSearchPhase = frame >= phase2At;
  const fullSearch = String.raw`re:Theorem\s+\d+`;
  const typedLen = inSearchPhase
    ? Math.floor(
        interpolate(frame, [phase2At, durationInFrames - 18], [0, fullSearch.length], {
          extrapolateLeft: 'clamp',
          extrapolateRight: 'clamp',
        }),
      )
    : 0;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14, alignItems: 'center' }}>
      <PinleafWindow
        heightPx={778}
        keyPressChip={
          inSearchPhase
            ? 'T: Auto-TOC  •  ⌘F: Regex  •  Space: Preview Scroll'
            : 'Pressed  ;  /  :  /  V  ->  2-Letter Vimium Hints'
        }
        panes={[
          {
            label: inSearchPhase
              ? 'Auto-TOC, Regex Search & Spacebar Scroll'
              : 'Vimium 2-Letter Home-Row Hints',
            active: true,
            pageProps: inSearchPhase
              ? {
                  pageNumber: 3,
                  focusSection: 'theorem',
                  scrollOffsetPx: Math.round(
                    interpolate(frame, [phase2At, durationInFrames], [0, 72], {
                      extrapolateLeft: 'clamp',
                      extrapolateRight: 'clamp',
                    }),
                  ),
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
                }
              : {
                  pageNumber: 1,
                  focusSection: 'intro',
                  hints: [
                    { code: 'as', xPercent: 18, yPercent: 17, active: true, typedPrefix: frame > 25 ? 'a' : undefined },
                    { code: 'df', xPercent: 39, yPercent: 17 },
                    { code: 'gh', xPercent: 12, yPercent: 34 },
                    { code: 'jk', xPercent: 36, yPercent: 34 },
                  ],
                },
          },
        ]}
        searchBar={
          inSearchPhase
            ? {
                query: fullSearch.slice(0, typedLen),
                matchLabel: 'Match 2 / 4 (n / N)',
              }
            : undefined
        }
        tocOverlay={
          inSearchPhase
            ? {
                query: 'Theorem',
                items: [
                  { level: 0, title: '1. Introduction', page: 1, inferred: true },
                  { level: 1, title: '3. Theoretical Bounds & Proofs', page: 3, inferred: true, selected: true },
                  { level: 1, title: '4. Empirical Benchmarks', page: 5, inferred: true },
                  { level: 0, title: '5. Variational Formulation', page: 7, inferred: true },
                ],
              }
            : undefined
        }
      />

      <div style={{ display: 'flex', gap: 14, fontFamily: FONT_SANS }}>
        {[
          { k: 'Space / ⇧Space', l: 'Screen Scroll' },
          { k: '<num>G / ⌥⌘G', l: 'Go to Page' },
          { k: 'm a / ` a', l: 'Named Marks' },
          { k: '⌘[ / ⌘]', l: 'Jump History' },
        ].map((chip) => (
          <div
            key={chip.k}
            style={{
              padding: '9px 16px',
              borderRadius: 11,
              backgroundColor: 'rgba(15, 23, 42, 0.9)',
              border: '1.5px solid rgba(96, 165, 250, 0.45)',
              fontSize: 18.5,
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
            return <Scene1CommandPaletteAndGoToPage />;
          case 1:
            return <Scene2SmartJumpOverview />;
          case 2:
            return <Scene3ThumbnailsStripAndGrid />;
          case 3:
          default:
            return <Scene4HintsTocSearchSpace />;
        }
      }}
    />
  );
};
