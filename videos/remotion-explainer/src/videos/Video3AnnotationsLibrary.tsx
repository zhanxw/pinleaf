import React from 'react';
import { interpolate, useCurrentFrame, useVideoConfig } from 'remotion';
import timingsJson from '../generated/timings.json';
import type { TimingsManifest } from '../types';
import { FONT_MONO, FONT_SANS } from '../components/StageFrame';
import { PinleafWindow } from '../components/PinleafWindow';
import { VideoSequenceShell } from '../components/VideoSequenceShell';

const manifest = timingsJson as TimingsManifest;
const VIDEO = manifest.videos.AnnotationsCategoriesLibrary;

const Scene1StandardAnnots: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const step2 = Math.round(durationInFrames * 0.25);
  const step3 = Math.round(durationInFrames * 0.5);
  const savedAt = Math.round(durationInFrames * 0.72);

  const highlights = [
    {
      topPx: 215,
      leftPercent: 8,
      widthPercent: 37,
      heightPx: 22,
      color: '#facc15',
      label: '⌘H Highlight',
      kind: 'highlight' as const,
    },
    ...(frame >= step2
      ? [
          {
            topPx: 292,
            leftPercent: 52,
            widthPercent: 39,
            heightPx: 20,
            color: '#60a5fa',
            label: 'U Underline',
            kind: 'underline' as const,
          },
        ]
      : []),
    ...(frame >= step3
      ? [
          {
            topPx: 332,
            leftPercent: 52,
            widthPercent: 33,
            heightPx: 18,
            color: '#f472b6',
            label: 'S Strikeout',
            kind: 'strikeout' as const,
          },
        ]
      : []),
  ];

  return (
    <PinleafWindow
      keyPressChip={
        frame < step2
          ? '⌘H Highlight  •  ⌘M Note'
          : frame < savedAt
          ? 'U Underline  •  S Strikeout  •  A Area Box'
          : '⌘S  →  Saved Atomic ISO-32000 /Annots'
      }
      panes={[
        {
          label: 'Pane #1 • Direct PDF /Annots Editing (⌘Z / ⇧⌘Z Undo/Redo)',
          active: true,
          pageProps: {
            pageNumber: 1,
            focusSection: 'intro',
            highlights,
            areaMarquee:
              frame >= step3 ? { x: 52, y: 120, w: 40, h: 118 } : null,
          },
        },
      ]}
      statusBanner={
        frame >= savedAt
          ? { text: 'Wrote standard /Highlight /Underline /StrikeOut /Square into PDF (Preview-compatible)', tone: 'ok' }
          : undefined
      }
    />
  );
};

const Scene2TypedCategories: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const phase = Math.floor(
    interpolate(frame, [0, durationInFrames], [0, 4], {
      extrapolateLeft: 'clamp',
      extrapolateRight: 'clamp',
    }),
  );

  const buckets = [
    { key: 't', label: 'Theorem', color: '#38bdf8', active: phase >= 0 },
    { key: 'm', label: 'Method', color: '#34d399', active: phase >= 1 },
    { key: 'd', label: 'Definition', color: '#facc15', active: phase >= 2 },
    { key: 'q', label: 'Question', color: '#fb7185', active: phase >= 3 },
    { key: 'r', label: 'Result', color: '#a78bfa', active: false },
    { key: 'c', label: 'Citation', color: '#fb923c', active: false },
    { key: 'e', label: 'Evidence', color: '#2dd4bf', active: false },
    { key: 'k', label: 'Key Idea', color: '#f472b6', active: false },
  ];

  return (
    <div style={{ display: 'flex', gap: 18, alignItems: 'stretch', width: 1520, height: 760 }}>
      <div style={{ flex: 1.55 }}>
        <PinleafWindow
          widthPx={940}
          heightPx={760}
          keyPressChip={`Pressed  H + ${buckets[Math.min(phase, 3)].key}  →  [${buckets[Math.min(phase, 3)].key}] ${buckets[Math.min(phase, 3)].label}`}
          panes={[
            {
              label: 'PDF /Subj (pinleaf-cat:<ch>) Persistence',
              active: true,
              pageProps: {
                pageNumber: 1,
                focusSection: 'intro',
                highlights: [
                  {
                    topPx: 210,
                    leftPercent: 8,
                    widthPercent: 37,
                    heightPx: 24,
                    color: '#38bdf8',
                    label: '[t] Theorem',
                  },
                  ...(phase >= 1
                    ? [
                        {
                          topPx: 300,
                          leftPercent: 8,
                          widthPercent: 35,
                          heightPx: 22,
                          color: '#34d399',
                          label: '[m] Method',
                        },
                      ]
                    : []),
                  ...(phase >= 2
                    ? [
                        {
                          topPx: 292,
                          leftPercent: 52,
                          widthPercent: 38,
                          heightPx: 22,
                          color: '#facc15',
                          label: '[d] Definition',
                        },
                      ]
                    : []),
                  ...(phase >= 3
                    ? [
                        {
                          topPx: 336,
                          leftPercent: 52,
                          widthPercent: 34,
                          heightPx: 22,
                          color: '#fb7185',
                          label: '[q] Question',
                        },
                      ]
                    : []),
                ],
              },
            },
          ]}
        />
      </div>

      {/* Side Reference Card: 26 Categories + config.json */}
      <div
        style={{
          flex: 1,
          borderRadius: 16,
          backgroundColor: 'rgba(15, 23, 42, 0.92)',
          border: '1px solid rgba(251, 191, 36, 0.38)',
          padding: 22,
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
          color: '#f8fafc',
        }}
      >
        <div>
          <div style={{ fontSize: 18, fontWeight: 700, color: '#fbbf24', marginBottom: 6 }}>
            26 Typed Highlight Buckets (H + a–z)
          </div>
          <div style={{ fontSize: 13.5, color: '#94a3b8', marginBottom: 14 }}>
            Stored cleanly in PDF <span style={{ fontFamily: FONT_MONO }}>/Annots /Subj</span> &amp;{' '}
            <span style={{ fontFamily: FONT_MONO }}>/C</span>:
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
            {buckets.map((b) => (
              <div
                key={b.key}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 9,
                  padding: '8px 10px',
                  borderRadius: 9,
                  backgroundColor: b.active ? `${b.color}24` : 'rgba(30, 41, 59, 0.7)',
                  border: `1.5px solid ${b.active ? b.color : 'rgba(148,163,184,0.18)'}`,
                }}
              >
                <span
                  style={{
                    fontFamily: FONT_MONO,
                    fontWeight: 800,
                    fontSize: 14,
                    padding: '2px 8px',
                    borderRadius: 6,
                    backgroundColor: b.color,
                    color: '#0f172a',
                  }}
                >
                  {b.key}
                </span>
                <span style={{ fontSize: 14, fontWeight: 600 }}>{b.label}</span>
              </div>
            ))}
          </div>
        </div>

        <div
          style={{
            borderRadius: 10,
            backgroundColor: '#020617',
            border: '1px solid #334155',
            padding: 14,
            fontFamily: FONT_MONO,
            fontSize: 12.5,
            lineHeight: 1.5,
            color: '#cbd5e1',
          }}
        >
          <div style={{ color: '#94a3b8', marginBottom: 4 }}>~/.config/pinleaf/config.json</div>
          <div>{`"highlight_categories": {`}</div>
          <div style={{ color: '#38bdf8' }}>{`  "t": { "label": "Main Theorem", "color": "#38bdf8" }`}</div>
          <div>{`}`}</div>
        </div>
      </div>
    </div>
  );
};

const Scene3SidebarFilter: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const filterTAt = Math.round(durationInFrames * 0.36);
  const filterQAt = Math.round(durationInFrames * 0.7);
  const activeFilter = frame < filterTAt ? null : frame < filterQAt ? 't' : 'm';

  const allItems = [
    {
      cat: 't',
      label: 'Theorem',
      color: '#38bdf8',
      page: 1,
      quote: 'Anchored stability preserves context entropy across k branches.',
      note: 'Compare with Section 4 proof.',
    },
    {
      cat: 'm',
      label: 'Method',
      color: '#34d399',
      page: 2,
      quote: 'Tile cache eviction stays bounded by O(V) for active panes.',
    },
    {
      cat: 't',
      label: 'Theorem',
      color: '#38bdf8',
      page: 4,
      quote: 'Wait-free rasterization updates SyncTeX without stream decode.',
    },
    {
      cat: 'q',
      label: 'Question',
      color: '#fb7185',
      page: 5,
      quote: 'What is overhead when 8 panes are split simultaneously?',
    },
  ];
  const visibleItems = activeFilter ? allItems.filter((i) => i.cat === activeFilter) : allItems;

  return (
    <PinleafWindow
      keyPressChip={
        activeFilter ? `Filtered Category: [${activeFilter}]` : '⇧⌘A  →  Notes & Category Sidebar'
      }
      sidebar={{
        activeFilter,
        filterChips: [
          { cat: 'all', label: 'All', color: '#e2e8f0', count: 4 },
          { cat: 't', label: '[t] Theorem', color: '#38bdf8', count: 2 },
          { cat: 'm', label: '[m] Method', color: '#34d399', count: 1 },
          { cat: 'q', label: '[q] Question', color: '#fb7185', count: 1 },
        ],
        items: visibleItems,
      }}
      panes={[
        {
          label: 'Pane #1 • Page 1',
          active: true,
          pageProps: { pageNumber: 1, focusSection: 'intro' },
        },
      ]}
    />
  );
};

const Scene4GlobalLibraryScholar: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const scholarAt = Math.round(durationInFrames * 0.62);
  const showScholar = frame >= scholarAt;

  return (
    <PinleafWindow
      keyPressChip={
        showScholar
          ? '⇧⌘K  →  Lookup DOI / Google Scholar'
          : '⇧⌘F  →  Global Highlight Search Across All PDFs'
      }
      panes={[
        {
          label: 'Pane #1 • Multi-Document Library',
          active: true,
          pageProps: { pageNumber: 1, focusSection: 'intro', activeSelectionText: showScholar },
        },
      ]}
      libraryModal={{
        heading: showScholar
          ? 'Quick-Open Recent (⌘P) & DOI / Scholar Action'
          : 'Global Highlight & Note Search (⇧⌘F)',
        query: showScholar ? '' : 'context entropy',
        rows: [
          {
            title: 'Attention_Preserving_Split_Graphs.pdf • p. 1',
            meta: '[t] Theorem — "Anchored stability preserves context entropy across k branches."',
            badge: 'p. 1 • Open',
            selected: true,
          },
          {
            title: 'Neural_Rendering_Benchmarks_2026.pdf • p. 6',
            meta: '[m] Method — "Frame-locked entropy decay keeps GPU VRAM flat."',
            badge: 'p. 6',
          },
          {
            title: 'Typst_SyncTeX_Incremental_Spec.pdf • p. 3',
            meta: '[d] Definition — "Byte-range source span mapping to PDF quad boxes."',
            badge: 'p. 3',
          },
        ],
      }}
      statusBanner={
        showScholar
          ? { text: 'Opening https://doi.org/10.1145/3597503.3623310 in browser', tone: 'ok' }
          : undefined
      }
    />
  );
};

export const Video3AnnotationsLibrary: React.FC = () => {
  return (
    <VideoSequenceShell
      video={VIDEO}
      transitionFrames={manifest.transitionFrames}
      renderScene={(idx) => {
        switch (idx) {
          case 0:
            return <Scene1StandardAnnots />;
          case 1:
            return <Scene2TypedCategories />;
          case 2:
            return <Scene3SidebarFilter />;
          case 3:
          default:
            return <Scene4GlobalLibraryScholar />;
        }
      }}
    />
  );
};
