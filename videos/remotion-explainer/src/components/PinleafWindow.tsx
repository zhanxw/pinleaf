import React from 'react';
import { Img, staticFile } from 'remotion';
import {
  AcademicPage,
  getThemePalette,
  type AcademicPageProps,
  type PinleafTheme,
} from './AcademicPage';
import { FONT_MONO, FONT_SANS } from './StageFrame';

export interface PaneSpec {
  label: string;
  frozen?: boolean;
  active?: boolean;
  portalFollower?: boolean;
  activePortalTarget?: string;
  pagePrefix?: string;
  autoScrollSpeed?: number;
  pageProps: AcademicPageProps;
}

export interface TrailPillSpec {
  id: number;
  frozen?: boolean;
  active?: boolean;
}

export interface PinleafWindowProps {
  title?: string;
  theme?: PinleafTheme;
  dirty?: boolean;
  presentationMode?: boolean;
  splitAxis?: 'horizontal' | 'vertical';
  trailNodes?: TrailPillSpec[];
  panes: PaneSpec[];
  zoomLabel?: string;
  autoReloadEnabled?: boolean;
  thumbnailsStrip?: {
    activePage: number;
    pages: Array<{ page: number; lockedTrail?: number; notes?: number; portals?: number }>;
  };
  thumbnailGridModal?: {
    selectedPage: number;
    pages: Array<{ page: number; section: string; lockedTrail?: number; notes?: number }>;
  };
  gotoPageModal?: {
    query: string;
    currentPage: number;
    pageCount: number;
  };
  brandMenuOpen?: boolean;
  aboutModal?: {
    version: string;
  };
  sidebar?: {
    activeFilter: string | null;
    items: Array<{
      cat: string;
      label: string;
      color: string;
      page: number;
      quote: string;
      note?: string;
    }>;
    filterChips?: Array<{ cat: string; label: string; color: string; count: number }>;
  };
  commandPalette?: {
    query: string;
    selectedIdx: number;
    items: Array<{ category: string; title: string; shortcut: string }>;
  };
  overviewCard?: {
    title: string;
    pageLabel: string;
    snippet: string;
    xPercent: number;
    yPercent: number;
    scale?: number;
    activeBtn?: 'jump' | 'split' | 'close';
  };
  tocOverlay?: {
    query: string;
    items: Array<{ level: number; title: string; page: number; inferred?: boolean; selected?: boolean }>;
  };
  searchBar?: {
    query: string;
    matchLabel: string;
  };
  libraryModal?: {
    heading: string;
    query?: string;
    rows: Array<{ title: string; meta: string; badge?: string; selected?: boolean }>;
  };
  keyPressChip?: string;
  statusBanner?: { text: string; tone?: 'ok' | 'info' };
  cursor?: { xPercent: number; yPercent: number; clicking?: boolean };
  widthPx?: number;
  heightPx?: number;
}

const ToolbarBtn: React.FC<{
  label: string;
  active?: boolean;
  amber?: boolean;
}> = ({ label, active = false, amber = false }) => (
  <div
    style={{
      padding: '5px 12px',
      borderRadius: 6,
      backgroundColor: amber ? '#d97706' : active ? '#2563eb' : '#27272a',
      color: '#ffffff',
      fontSize: 16,
      fontWeight: amber || active ? 800 : 600,
      whiteSpace: 'nowrap',
    }}
  >
    {label}
  </div>
);

const MiniPageThumb: React.FC<{ page: number; dark?: boolean; lines?: number }> = ({
  page,
  dark = true,
  lines = 5,
}) => (
  <div
    style={{
      width: '100%',
      height: '100%',
      backgroundColor: dark ? '#fafafa' : '#ffffff',
      borderRadius: 4,
      padding: '7px 8px',
      display: 'flex',
      flexDirection: 'column',
      justifyContent: 'space-between',
      boxSizing: 'border-box',
    }}
  >
    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <div
        style={{
          height: 5,
          width: `${64 + ((page * 11) % 24)}%`,
          borderRadius: 2,
          backgroundColor: '#1e293b',
        }}
      />
      {Array.from({ length: lines }).map((_, idx) => (
        <div
          key={idx}
          style={{
            height: 3,
            width: `${74 + ((page * 7 + idx * 13) % 24)}%`,
            borderRadius: 2,
            backgroundColor: idx === 2 && page % 2 === 1 ? '#93c5fd' : '#cbd5e1',
          }}
        />
      ))}
    </div>
    <div style={{ display: 'flex', gap: 4 }}>
      <div
        style={{
          flex: 1,
          height: page % 2 === 0 ? 20 : 12,
          borderRadius: 3,
          backgroundColor: page % 2 === 0 ? '#dbeafe' : '#e2e8f0',
          border: page % 2 === 0 ? '1px solid #93c5fd' : 'none',
        }}
      />
    </div>
  </div>
);

export const PinleafWindow: React.FC<PinleafWindowProps> = ({
  title = 'Pinleaf — Attention_Preserving_Split_Graphs.pdf',
  theme = 'light',
  dirty = false,
  presentationMode = false,
  splitAxis = 'horizontal',
  trailNodes,
  panes,
  zoomLabel = '118%',
  autoReloadEnabled = false,
  thumbnailsStrip,
  thumbnailGridModal,
  gotoPageModal,
  brandMenuOpen = false,
  aboutModal,
  sidebar,
  commandPalette,
  overviewCard,
  tocOverlay,
  searchBar,
  libraryModal,
  keyPressChip,
  statusBanner,
  cursor,
  widthPx = 1860,
  heightPx = 852,
}) => {
  const pal = getThemePalette(theme);
  const darkChrome = theme === 'dark' || theme === 'custom';
  const themeLabel = theme.charAt(0).toUpperCase() + theme.slice(1);
  const notesCount = sidebar ? sidebar.items.length : 1;
  const windowTitle = dirty && !title.endsWith('*') ? `${title} *` : title;

  if (presentationMode) {
    return (
      <div
        style={{
          position: 'relative',
          width: widthPx,
          height: heightPx,
          backgroundColor: '#05070c',
          borderRadius: 16,
          padding: 24,
          border: '1px solid rgba(148, 163, 184, 0.25)',
          boxShadow: '0 30px 90px rgba(0, 0, 0, 0.7)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          overflow: 'hidden',
        }}
      >
        <div style={{ width: '94%', height: '94%' }}>
          <AcademicPage {...panes[0].pageProps} focusSection="slide" />
        </div>
        <div
          style={{
            position: 'absolute',
            top: 16,
            right: 22,
            padding: '7px 16px',
            borderRadius: 999,
            backgroundColor: 'rgba(15, 23, 42, 0.9)',
            border: '1.5px solid rgba(192, 132, 252, 0.65)',
            fontFamily: FONT_MONO,
            fontSize: 18,
            fontWeight: 800,
            color: '#e9d5ff',
          }}
        >
          Presentation Mode (F5) • Esc to Exit
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        position: 'relative',
        width: widthPx,
        height: heightPx,
        borderRadius: 16,
        overflow: 'hidden',
        backgroundColor: pal.canvasBg,
        border: darkChrome
          ? '1px solid rgba(148, 163, 184, 0.28)'
          : '1px solid rgba(255, 255, 255, 0.35)',
        boxShadow: '0 28px 85px rgba(0, 0, 0, 0.55)',
        display: 'flex',
        flexDirection: 'column',
        fontFamily: FONT_SANS,
      }}
    >
      {/* macOS Titlebar */}
      <div
        style={{
          height: 38,
          backgroundColor: darkChrome ? '#1e293b' : '#f1f5f9',
          borderBottom: `1px solid ${pal.border}`,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 18px',
          flexShrink: 0,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
          <div style={{ width: 13, height: 13, borderRadius: 99, backgroundColor: '#ef4444' }} />
          <div style={{ width: 13, height: 13, borderRadius: 99, backgroundColor: '#f59e0b' }} />
          <div style={{ width: 13, height: 13, borderRadius: 99, backgroundColor: '#22c55e' }} />
        </div>
        <div
          style={{
            fontSize: 17.5,
            fontWeight: 700,
            color: darkChrome ? '#e2e8f0' : '#334155',
          }}
        >
          {windowTitle}
        </div>
        <div style={{ width: 56 }} />
      </div>

      {/* Redesigned Compact Dark Toolbar */}
      <div
        style={{
          minHeight: 46,
          backgroundColor: '#18181b',
          borderBottom: '1px solid #27272a',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '6px 14px',
          gap: 10,
          flexShrink: 0,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 7,
              padding: '5px 12px',
              borderRadius: 6,
              backgroundColor: brandMenuOpen ? '#2563eb' : '#1e3a8a',
              color: '#e0f2fe',
              fontSize: 16.5,
              fontWeight: 800,
            }}
          >
            <Img
              src={staticFile('pinleaf-icon.png')}
              style={{ width: 20, height: 20, borderRadius: 4 }}
            />
            <span>Pinleaf</span>
          </div>
          <ToolbarBtn label="Open" />
          <ToolbarBtn
            label="Recent"
            active={Boolean(libraryModal?.heading.includes('Recent'))}
          />
          {dirty && <ToolbarBtn label="Undo" />}
          <ToolbarBtn label={dirty ? 'Save *' : 'Save'} amber={dirty} />

          {trailNodes && trailNodes.length > 0 && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 6,
                marginLeft: 6,
                padding: '3px 9px',
                borderRadius: 7,
                backgroundColor: '#09090b',
                border: '1px solid #27272a',
              }}
            >
              {trailNodes.map((n, idx) => (
                <React.Fragment key={n.id}>
                  {idx > 0 && <span style={{ color: '#71717a', fontSize: 14 }}>→</span>}
                  <span
                    style={{
                      padding: '2px 8px',
                      borderRadius: 5,
                      fontFamily: FONT_MONO,
                      fontSize: 14.5,
                      fontWeight: 800,
                      backgroundColor: n.frozen
                        ? '#f59e0b'
                        : n.active
                        ? '#2563eb'
                        : '#27272a',
                      color: n.frozen ? '#18181b' : '#ffffff',
                    }}
                  >
                    #{n.id} {n.frozen ? 'LOCK' : n.active ? 'ACTIVE' : ''}
                  </span>
                </React.Fragment>
              ))}
            </div>
          )}

          {autoReloadEnabled && (
            <div
              style={{
                padding: '4px 10px',
                borderRadius: 6,
                backgroundColor: '#0e7490',
                color: '#ecfeff',
                fontFamily: FONT_MONO,
                fontSize: 14.5,
                fontWeight: 700,
              }}
            >
              Auto-Reload ON
            </div>
          )}
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <ToolbarBtn
            label="Pages"
            active={Boolean(thumbnailsStrip || thumbnailGridModal)}
          />
          <ToolbarBtn label="TOC" active={Boolean(tocOverlay)} />
          <ToolbarBtn label="Find" active={Boolean(searchBar)} />
          <ToolbarBtn label={`Notes (${notesCount})`} active={Boolean(sidebar)} />
          <ToolbarBtn label={themeLabel} active={theme !== 'light'} />
          <ToolbarBtn label="/ Cmds" active={Boolean(commandPalette)} />
        </div>
      </div>

      {/* Main Content Body */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden', position: 'relative' }}>
        {/* Left Virtualized Thumbnail Strip */}
        {thumbnailsStrip && (
          <div
            style={{
              width: 210,
              backgroundColor: '#18181b',
              borderRight: '1px solid #27272a',
              display: 'flex',
              flexDirection: 'column',
              flexShrink: 0,
            }}
          >
            <div
              style={{
                height: 36,
                padding: '0 12px',
                borderBottom: '1px solid #27272a',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                fontSize: 13.5,
                fontWeight: 800,
                color: '#d4d4d8',
              }}
            >
              <span>PAGES ({thumbnailsStrip.pages.length})</span>
              <span style={{ color: '#a1a1aa' }}>x</span>
            </div>
            <div
              style={{
                flex: 1,
                padding: 10,
                display: 'flex',
                flexDirection: 'column',
                gap: 10,
                overflow: 'hidden',
              }}
            >
              {thumbnailsStrip.pages.map((tp) => {
                const isActive = tp.page === thumbnailsStrip.activePage;
                return (
                  <div
                    key={tp.page}
                    style={{
                      padding: 8,
                      borderRadius: 8,
                      backgroundColor: isActive ? '#1e3a8a55' : '#27272a',
                      border: isActive ? '2px solid #3b82f6' : '1px solid #3f3f46',
                      display: 'flex',
                      flexDirection: 'column',
                      gap: 6,
                    }}
                  >
                    <div style={{ height: 112 }}>
                      <MiniPageThumb page={tp.page} />
                    </div>
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        fontSize: 13.5,
                        fontFamily: FONT_MONO,
                        fontWeight: 700,
                        color: isActive ? '#93c5fd' : '#d4d4d8',
                      }}
                    >
                      <span>p.{tp.page}</span>
                      <div style={{ display: 'flex', gap: 4 }}>
                        {tp.lockedTrail && (
                          <span
                            style={{
                              padding: '1px 6px',
                              borderRadius: 4,
                              backgroundColor: '#f59e0b',
                              color: '#18181b',
                              fontSize: 12,
                              fontWeight: 800,
                            }}
                          >
                            #{tp.lockedTrail} LOCK
                          </span>
                        )}
                        {tp.notes && tp.notes > 0 && (
                          <span
                            style={{
                              padding: '1px 6px',
                              borderRadius: 4,
                              backgroundColor: '#3f3f46',
                              color: '#fde047',
                              fontSize: 12,
                            }}
                          >
                            {tp.notes}n
                          </span>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
            <div
              style={{
                padding: '6px 10px',
                borderTop: '1px solid #27272a',
                fontSize: 12.5,
                color: '#a1a1aa',
                textAlign: 'center',
              }}
            >
              Click: Jump • Cmd+Click: Split
            </div>
          </div>
        )}

        {/* Panes Container + Overlaid Toast */}
        <div
          style={{
            position: 'relative',
            flex: 1,
            display: 'flex',
            flexDirection: splitAxis === 'vertical' ? 'column' : 'row',
            gap: 10,
            padding: 10,
            overflow: 'hidden',
          }}
        >
          {panes.map((pane, idx) => (
            <div
              key={idx}
              style={{
                flex: 1,
                display: 'flex',
                flexDirection: 'column',
                borderRadius: 10,
                border: pane.frozen
                  ? '2.5px solid #f59e0b'
                  : pane.active
                  ? '2.5px solid #3b82f6'
                  : pane.portalFollower
                  ? '2.5px solid #0ea5e9'
                  : `1px solid ${pal.border}`,
                backgroundColor: pal.canvasBg,
                overflow: 'hidden',
              }}
            >
              {/* Actual Pinleaf Pane Header */}
              <div
                style={{
                  height: 38,
                  backgroundColor: darkChrome ? '#18181b' : '#f4f4f5',
                  borderBottom: `1px solid ${darkChrome ? '#27272a' : '#e4e4e7'}`,
                  padding: '0 10px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  fontSize: 15,
                  fontWeight: 700,
                  flexShrink: 0,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
                  <span
                    style={{
                      padding: '2px 8px',
                      borderRadius: 5,
                      fontFamily: FONT_MONO,
                      fontSize: 13.5,
                      fontWeight: 800,
                      backgroundColor: pane.frozen
                        ? '#f59e0b'
                        : pane.active
                        ? '#2563eb'
                        : '#e4e4e7',
                      color: pane.frozen
                        ? '#18181b'
                        : pane.active
                        ? '#ffffff'
                        : '#27272a',
                    }}
                  >
                    #{idx + 1} {pane.frozen ? 'LOCKED [F]' : 'Freeze [F]'}
                  </span>

                  {pane.portalFollower && (
                    <span
                      style={{
                        padding: '2px 8px',
                        borderRadius: 5,
                        backgroundColor: '#0369a1',
                        color: '#e0f2fe',
                        fontSize: 13.5,
                        fontWeight: 700,
                      }}
                    >
                      Portal [⇧P]{pane.activePortalTarget ? ` • ${pane.activePortalTarget}` : ''}
                    </span>
                  )}

                  <span
                    style={{
                      padding: '2px 8px',
                      borderRadius: 5,
                      fontFamily: FONT_MONO,
                      fontSize: 13.5,
                      backgroundColor: pane.pagePrefix ? '#1d4ed8' : '#e4e4e7',
                      color: pane.pagePrefix ? '#ffffff' : '#18181b',
                    }}
                  >
                    {pane.pagePrefix
                      ? `Go: ${pane.pagePrefix}_ (${pane.pageProps.pageNumber}/8)`
                      : `p.${pane.pageProps.pageNumber}/8`}
                  </span>

                  <span
                    style={{
                      padding: '2px 8px',
                      borderRadius: 5,
                      fontFamily: FONT_MONO,
                      fontSize: 13.5,
                      backgroundColor: '#e4e4e7',
                      color: '#27272a',
                    }}
                  >
                    {zoomLabel}
                  </span>

                  <span
                    style={{
                      color: darkChrome ? '#e4e4e7' : '#3f3f46',
                      fontSize: 14,
                      fontWeight: 600,
                    }}
                  >
                    {pane.label}
                  </span>
                </div>

                <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                  {pane.autoScrollSpeed && (
                    <span
                      style={{
                        padding: '2px 8px',
                        borderRadius: 5,
                        fontFamily: FONT_MONO,
                        fontSize: 13.5,
                        backgroundColor: '#047857',
                        color: '#ffffff',
                      }}
                    >
                      Auto {pane.autoScrollSpeed}px/s
                    </span>
                  )}
                  {['Split R', 'Split D', 'x'].map((b) => (
                    <span
                      key={b}
                      style={{
                        padding: '2px 7px',
                        borderRadius: 5,
                        backgroundColor: '#e4e4e7',
                        color: '#27272a',
                        fontSize: 13,
                        fontWeight: 700,
                      }}
                    >
                      {b}
                    </span>
                  ))}
                </div>
              </div>

              <div style={{ flex: 1, padding: '8px 6px', overflow: 'hidden' }}>
                <AcademicPage {...pane.pageProps} theme={pane.pageProps.theme ?? theme} />
              </div>
            </div>
          ))}

          {/* Floating Bottom-Right Toast inside Canvas */}
          {statusBanner && (
            <div
              style={{
                position: 'absolute',
                bottom: 16,
                right: 16,
                padding: '9px 16px',
                borderRadius: 9,
                backgroundColor:
                  statusBanner.tone === 'ok' ? '#14532d' : '#18181b',
                border:
                  statusBanner.tone === 'ok'
                    ? '1.5px solid #22c55e'
                    : '1.5px solid #52525b',
                color: '#ffffff',
                fontFamily: FONT_MONO,
                fontSize: 17,
                fontWeight: 700,
                boxShadow: '0 10px 28px rgba(0,0,0,0.45)',
                display: 'flex',
                alignItems: 'center',
                gap: 12,
                zIndex: 20,
              }}
            >
              <span>{statusBanner.text}</span>
              <span style={{ color: '#d4d4d8', fontSize: 15 }}>x</span>
            </div>
          )}

          {/* KeyPress Callout (Bottom-Left) */}
          {keyPressChip && (
            <div
              style={{
                position: 'absolute',
                bottom: 16,
                left: 16,
                padding: '9px 16px',
                borderRadius: 10,
                backgroundColor: 'rgba(2, 6, 23, 0.94)',
                border: '2px solid #34d399',
                fontFamily: FONT_MONO,
                fontSize: 19,
                fontWeight: 800,
                color: '#34d399',
                boxShadow: '0 8px 24px rgba(0,0,0,0.45)',
                zIndex: 20,
              }}
            >
              {keyPressChip}
            </div>
          )}
        </div>

        {/* Right Notes & Categories Sidebar */}
        {sidebar && (
          <div
            style={{
              width: 440,
              backgroundColor: darkChrome ? '#0f172a' : '#f8fafc',
              borderLeft: `1px solid ${pal.border}`,
              padding: 16,
              display: 'flex',
              flexDirection: 'column',
              gap: 11,
              flexShrink: 0,
            }}
          >
            <div
              style={{
                fontSize: 19,
                fontWeight: 800,
                color: pal.heading,
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
              }}
            >
              <span>Notes &amp; Categories (⇧⌘A)</span>
              <span style={{ fontSize: 15.5, color: pal.textMuted }}>
                {sidebar.items.length} shown
              </span>
            </div>

            {sidebar.filterChips && (
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 7 }}>
                {sidebar.filterChips.map((chip) => {
                  const selected =
                    (chip.cat === 'all' && sidebar.activeFilter === null) ||
                    chip.cat === sidebar.activeFilter;
                  return (
                    <div
                      key={chip.cat}
                      style={{
                        padding: '4px 10px',
                        borderRadius: 999,
                        fontSize: 15.5,
                        fontWeight: 800,
                        backgroundColor: selected ? chip.color : 'rgba(148,163,184,0.18)',
                        color: selected ? '#0f172a' : pal.textPrimary,
                        border: `1.5px solid ${selected ? chip.color : pal.border}`,
                      }}
                    >
                      {chip.label} ({chip.count})
                    </div>
                  );
                })}
              </div>
            )}

            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {sidebar.items.map((item, idx) => (
                <div
                  key={idx}
                  style={{
                    padding: 12,
                    borderRadius: 10,
                    backgroundColor: darkChrome ? '#1e293b' : '#ffffff',
                    border: `1px solid ${pal.border}`,
                    borderLeft: `5px solid ${item.color}`,
                  }}
                >
                  <div
                    style={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      marginBottom: 5,
                      fontSize: 15.5,
                      fontWeight: 800,
                    }}
                  >
                    <span
                      style={{
                        padding: '2px 8px',
                        borderRadius: 5,
                        backgroundColor: `${item.color}33`,
                        color: darkChrome ? '#f8fafc' : '#0f172a',
                      }}
                    >
                      [{item.cat}] {item.label}
                    </span>
                    <span style={{ color: pal.textMuted }}>p. {item.page}</span>
                  </div>
                  <div style={{ fontSize: 16.5, color: pal.textPrimary, lineHeight: 1.35 }}>
                    “{item.quote}”
                  </div>
                  {item.note && (
                    <div
                      style={{
                        marginTop: 5,
                        fontSize: 15,
                        color: pal.textMuted,
                        fontStyle: 'italic',
                      }}
                    >
                      Note: {item.note}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Search Bar Overlay */}
        {searchBar && (
          <div
            style={{
              position: 'absolute',
              top: 16,
              right: 22,
              padding: '10px 18px',
              borderRadius: 11,
              backgroundColor: 'rgba(15, 23, 42, 0.96)',
              border: '2px solid #60a5fa',
              color: '#f8fafc',
              display: 'flex',
              alignItems: 'center',
              gap: 14,
              fontFamily: FONT_MONO,
              fontSize: 19.5,
              fontWeight: 700,
              boxShadow: '0 12px 28px rgba(0,0,0,0.45)',
            }}
          >
            <span style={{ color: '#93c5fd', fontWeight: 800 }}>⌘F</span>
            <span>{searchBar.query}</span>
            <span style={{ color: '#34d399', fontSize: 17 }}>{searchBar.matchLabel}</span>
          </div>
        )}

        {/* Command Palette Overlay (/) */}
        {commandPalette && (
          <div
            style={{
              position: 'absolute',
              top: 26,
              left: '50%',
              transform: 'translateX(-50%)',
              width: 1020,
              borderRadius: 16,
              backgroundColor: 'rgba(24, 24, 27, 0.97)',
              border: '2px solid rgba(96, 165, 250, 0.72)',
              boxShadow: '0 24px 60px rgba(0, 0, 0, 0.65)',
              padding: 20,
              color: '#f8fafc',
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                padding: '11px 16px',
                borderRadius: 10,
                backgroundColor: '#09090b',
                border: '1.5px solid #3f3f46',
                marginBottom: 14,
                fontFamily: FONT_MONO,
                fontSize: 22,
                fontWeight: 700,
              }}
            >
              <span>
                <span style={{ color: '#60a5fa', fontWeight: 800 }}>/ </span>
                {commandPalette.query}
                <span style={{ color: '#60a5fa' }}>|</span>
              </span>
              <span style={{ fontSize: 15.5, color: '#a1a1aa' }}>
                Up/Down Navigate · Enter Run · Esc Close
              </span>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              {commandPalette.items.map((item, i) => {
                const selected = i === commandPalette.selectedIdx;
                return (
                  <div
                    key={i}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: '11px 14px',
                      borderRadius: 9,
                      backgroundColor: selected ? 'rgba(59, 130, 246, 0.28)' : 'transparent',
                      border: selected ? '1.5px solid rgba(96, 165, 250, 0.6)' : '1px solid transparent',
                    }}
                  >
                    <div>
                      <span
                        style={{
                          fontSize: 14.5,
                          fontWeight: 800,
                          color: '#93c5fd',
                          marginRight: 12,
                          textTransform: 'uppercase',
                        }}
                      >
                        {item.category}
                      </span>
                      <span style={{ fontSize: 21, fontWeight: 700 }}>{item.title}</span>
                    </div>
                    <span
                      style={{
                        fontFamily: FONT_MONO,
                        fontSize: 18,
                        fontWeight: 800,
                        padding: '4px 12px',
                        borderRadius: 7,
                        backgroundColor: '#27272a',
                        color: '#fbbf24',
                      }}
                    >
                      {item.shortcut}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* Go-to-Page Modal */}
        {gotoPageModal && (
          <div
            style={{
              position: 'absolute',
              inset: 0,
              backgroundColor: 'rgba(9, 9, 11, 0.45)',
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'flex-start',
              paddingTop: 72,
              zIndex: 24,
            }}
          >
            <div
              style={{
                width: 480,
                borderRadius: 12,
                backgroundColor: '#18181b',
                border: '2px solid #3f3f46',
                boxShadow: '0 22px 54px rgba(0, 0, 0, 0.65)',
                padding: 18,
                color: '#f4f4f5',
                display: 'flex',
                flexDirection: 'column',
                gap: 12,
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span style={{ fontSize: 20, fontWeight: 800, color: '#ffffff' }}>Go to Page</span>
                <span style={{ fontFamily: FONT_MONO, fontSize: 14.5, color: '#a1a1aa' }}>
                  Enter Jump · Cmd+Enter Split · Esc
                </span>
              </div>
              <div
                style={{
                  padding: '10px 14px',
                  borderRadius: 8,
                  backgroundColor: '#09090b',
                  border: '1.5px solid #2563eb',
                  fontFamily: FONT_MONO,
                  fontSize: 22,
                  fontWeight: 700,
                  color: gotoPageModal.query ? '#ffffff' : '#71717a',
                }}
              >
                {gotoPageModal.query
                  ? `${gotoPageModal.query}|`
                  : `Page 1–${gotoPageModal.pageCount} (now ${gotoPageModal.currentPage})|`}
              </div>
            </div>
          </div>
        )}

        {/* 4-Column Thumbnail Grid Overview Modal */}
        {thumbnailGridModal && (
          <div
            style={{
              position: 'absolute',
              inset: 0,
              backgroundColor: 'rgba(9, 9, 11, 0.6)',
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'center',
              zIndex: 24,
            }}
          >
            <div
              style={{
                width: 1120,
                borderRadius: 14,
                backgroundColor: '#18181b',
                border: '2px solid #3f3f46',
                boxShadow: '0 28px 72px rgba(0, 0, 0, 0.72)',
                overflow: 'hidden',
                color: '#f4f4f5',
              }}
            >
              <div
                style={{
                  padding: '12px 18px',
                  borderBottom: '1px solid #27272a',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                }}
              >
                <span style={{ fontSize: 20, fontWeight: 800, color: '#ffffff' }}>
                  Page Overview ({thumbnailGridModal.pages.length} pages)
                </span>
                <span style={{ fontFamily: FONT_MONO, fontSize: 15, color: '#a1a1aa' }}>
                  Arrows/hjkl Move · Enter Jump · Cmd+Enter Split · Esc Close
                </span>
              </div>
              <div
                style={{
                  padding: 16,
                  display: 'grid',
                  gridTemplateColumns: 'repeat(4, 1fr)',
                  gap: 14,
                }}
              >
                {thumbnailGridModal.pages.map((tp) => {
                  const selected = tp.page === thumbnailGridModal.selectedPage;
                  return (
                    <div
                      key={tp.page}
                      style={{
                        padding: 10,
                        borderRadius: 10,
                        backgroundColor: selected ? '#1e3a8a66' : '#27272a',
                        border: selected ? '2.5px solid #3b82f6' : '1px solid #3f3f46',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 8,
                      }}
                    >
                      <div style={{ height: 150 }}>
                        <MiniPageThumb page={tp.page} lines={6} />
                      </div>
                      <div
                        style={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                          fontSize: 15,
                          fontWeight: 700,
                        }}
                      >
                        <span style={{ color: selected ? '#93c5fd' : '#f4f4f5' }}>
                          p.{tp.page} · {tp.section}
                        </span>
                        <div style={{ display: 'flex', gap: 5 }}>
                          {tp.lockedTrail && (
                            <span
                              style={{
                                padding: '2px 6px',
                                borderRadius: 4,
                                backgroundColor: '#f59e0b',
                                color: '#18181b',
                                fontFamily: FONT_MONO,
                                fontSize: 12.5,
                                fontWeight: 800,
                              }}
                            >
                              #{tp.lockedTrail} LOCK
                            </span>
                          )}
                          {tp.notes && tp.notes > 0 && (
                            <span
                              style={{
                                padding: '2px 6px',
                                borderRadius: 4,
                                backgroundColor: '#3f3f46',
                                color: '#fde047',
                                fontFamily: FONT_MONO,
                                fontSize: 12.5,
                              }}
                            >
                              {tp.notes}n
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        )}

        {/* TOC Overlay */}
        {tocOverlay && (
          <div
            style={{
              position: 'absolute',
              top: 24,
              left: 36,
              width: 640,
              borderRadius: 15,
              backgroundColor: 'rgba(15, 23, 42, 0.97)',
              border: '2px solid rgba(52, 211, 153, 0.65)',
              boxShadow: '0 22px 54px rgba(0,0,0,0.6)',
              padding: 18,
              color: '#f8fafc',
            }}
          >
            <div
              style={{
                fontFamily: FONT_MONO,
                fontSize: 19,
                fontWeight: 700,
                padding: '9px 14px',
                borderRadius: 9,
                backgroundColor: '#020617',
                border: '1px solid #334155',
                marginBottom: 12,
              }}
            >
              TOC Filter: <span style={{ color: '#34d399' }}>{tocOverlay.query || 'all sections'}</span>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
              {tocOverlay.items.map((row, idx) => (
                <div
                  key={idx}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    padding: '8px 12px',
                    paddingLeft: 12 + row.level * 18,
                    borderRadius: 8,
                    backgroundColor: row.selected ? 'rgba(16, 185, 129, 0.26)' : 'transparent',
                    fontSize: 19,
                    fontWeight: 600,
                  }}
                >
                  <span>
                    {row.title}{' '}
                    {row.inferred && (
                      <span style={{ fontSize: 14.5, color: '#94a3b8' }}>(Auto-TOC)</span>
                    )}
                  </span>
                  <span style={{ fontFamily: FONT_MONO, color: '#94a3b8' }}>p.{row.page}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Library / Global Search / Extensions Modal */}
        {libraryModal && (
          <div
            style={{
              position: 'absolute',
              top: 32,
              left: '50%',
              transform: 'translateX(-50%)',
              width: 1120,
              borderRadius: 16,
              backgroundColor: 'rgba(15, 23, 42, 0.97)',
              border: '2px solid rgba(251, 191, 36, 0.72)',
              boxShadow: '0 26px 68px rgba(0,0,0,0.65)',
              padding: 20,
              color: '#f8fafc',
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: 14,
              }}
            >
              <span style={{ fontSize: 23, fontWeight: 800, color: '#fde68a' }}>
                {libraryModal.heading}
              </span>
              {libraryModal.query !== undefined && (
                <span
                  style={{
                    fontFamily: FONT_MONO,
                    fontSize: 19,
                    fontWeight: 700,
                    padding: '5px 12px',
                    borderRadius: 8,
                    backgroundColor: '#020617',
                    border: '1px solid #334155',
                  }}
                >
                  Filter: {libraryModal.query}
                </span>
              )}
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {libraryModal.rows.map((r, i) => (
                <div
                  key={i}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    gap: 16,
                    padding: '12px 15px',
                    borderRadius: 10,
                    backgroundColor: r.selected ? 'rgba(251, 191, 36, 0.2)' : 'rgba(30, 41, 59, 0.72)',
                    border: r.selected
                      ? '1.5px solid rgba(251, 191, 36, 0.7)'
                      : '1px solid rgba(148,163,184,0.2)',
                  }}
                >
                  <div>
                    <div style={{ fontSize: 20, fontWeight: 700 }}>{r.title}</div>
                    <div style={{ fontSize: 17, color: '#cbd5e1', marginTop: 3 }}>{r.meta}</div>
                  </div>
                  {r.badge && (
                    <span
                      style={{
                        fontFamily: FONT_MONO,
                        fontSize: 18,
                        fontWeight: 800,
                        padding: '4px 12px',
                        borderRadius: 7,
                        backgroundColor: '#1e293b',
                        color: '#fbbf24',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {r.badge}
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Top-Left [Pinleaf] Dropdown Menu */}
        {brandMenuOpen && (
          <div
            style={{
              position: 'absolute',
              top: 4,
              left: 14,
              width: 280,
              borderRadius: 10,
              backgroundColor: '#18181b',
              border: '1.5px solid #3f3f46',
              boxShadow: '0 20px 50px rgba(0, 0, 0, 0.65)',
              padding: '6px 0',
              color: '#f4f4f5',
              fontSize: 16.5,
              zIndex: 28,
            }}
          >
            {[
              { label: 'About Pinleaf', hint: 'v0.2.2', highlight: true },
              null,
              { label: 'Open Built-in Demo', hint: '' },
              { label: 'Save PDF As...', hint: '' },
              { label: 'Commands & Shortcuts', hint: '/' },
              null,
              { label: 'Quit Pinleaf', hint: 'Cmd+Q' },
            ].map((item, idx) =>
              item === null ? (
                <div key={idx} style={{ height: 1, margin: '5px 0', backgroundColor: '#27272a' }} />
              ) : (
                <div
                  key={item.label}
                  style={{
                    padding: '8px 14px',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    backgroundColor: item.highlight ? '#27272a' : 'transparent',
                    fontWeight: item.highlight ? 700 : 500,
                  }}
                >
                  <span>{item.label}</span>
                  {item.hint && (
                    <span style={{ fontFamily: FONT_MONO, fontSize: 14.5, color: '#a1a1aa' }}>
                      {item.hint}
                    </span>
                  )}
                </div>
              ),
            )}
          </div>
        )}

        {/* Centered About Pinleaf Modal */}
        {aboutModal && (
          <div
            style={{
              position: 'absolute',
              inset: 0,
              backgroundColor: 'rgba(9, 9, 11, 0.56)',
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'center',
              zIndex: 28,
            }}
          >
            <div
              style={{
                width: 480,
                borderRadius: 14,
                backgroundColor: '#18181b',
                border: '1.5px solid #3f3f46',
                boxShadow: '0 28px 70px rgba(0, 0, 0, 0.72)',
                overflow: 'hidden',
                color: '#f4f4f5',
              }}
            >
              <div
                style={{
                  padding: '10px 16px',
                  borderBottom: '1px solid #27272a',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  fontSize: 16,
                }}
              >
                <span style={{ fontWeight: 800, color: '#ffffff' }}>About Pinleaf</span>
                <span
                  style={{
                    padding: '3px 10px',
                    borderRadius: 5,
                    backgroundColor: '#27272a',
                    color: '#e4e4e7',
                    fontSize: 14.5,
                  }}
                >
                  Close [Esc]
                </span>
              </div>
              <div
                style={{
                  padding: '24px 28px',
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'center',
                  gap: 10,
                }}
              >
                <Img
                  src={staticFile('pinleaf-icon.png')}
                  style={{ width: 68, height: 68, borderRadius: 14 }}
                />
                <div style={{ fontSize: 26, fontWeight: 800, color: '#ffffff' }}>Pinleaf</div>
                <div
                  style={{
                    padding: '4px 14px',
                    borderRadius: 6,
                    backgroundColor: '#1e3a8a',
                    color: '#e0f2fe',
                    fontFamily: FONT_MONO,
                    fontSize: 16,
                    fontWeight: 700,
                  }}
                >
                  Version {aboutModal.version} (aarch64-macos)
                </div>
                <div style={{ fontSize: 16.5, color: '#d4d4d8', marginTop: 4 }}>
                  Freeze-and-branch multi-view academic PDF reader
                </div>
                <div style={{ fontSize: 15, color: '#a1a1aa' }}>
                  Pure Rust + Metal GPU  |  gpui-ce  |  hayro  |  lopdf
                </div>
              </div>
              <div
                style={{
                  padding: '12px 18px',
                  borderTop: '1px solid #27272a',
                  display: 'flex',
                  justifyContent: 'flex-end',
                  gap: 10,
                  fontSize: 15.5,
                  fontWeight: 700,
                }}
              >
                <span style={{ padding: '6px 12px', borderRadius: 6, backgroundColor: '#27272a' }}>
                  Copy Version
                </span>
                <span style={{ padding: '6px 12px', borderRadius: 6, backgroundColor: '#27272a' }}>
                  GitHub
                </span>
                <span style={{ padding: '6px 16px', borderRadius: 6, backgroundColor: '#2563eb' }}>
                  OK
                </span>
              </div>
            </div>
          </div>
        )}

        {/* Floating Overview Card with Keyboard Shortcut Buttons */}
        {overviewCard && (
          <div
            style={{
              position: 'absolute',
              left: `${overviewCard.xPercent}%`,
              top: `${overviewCard.yPercent}%`,
              width: 620,
              transform: `scale(${overviewCard.scale ?? 1})`,
              borderRadius: 12,
              backgroundColor: '#18181b',
              border: '2px solid #3b82f6',
              boxShadow: '0 22px 54px rgba(0,0,0,0.62)',
              overflow: 'hidden',
              color: '#f4f4f5',
              zIndex: 15,
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                gap: 10,
                padding: '9px 14px',
                borderBottom: '1px solid #27272a',
              }}
            >
              <span style={{ fontSize: 17, fontWeight: 700, color: '#f4f4f5' }}>
                Overview: {overviewCard.title} ({overviewCard.pageLabel})
              </span>
              <div style={{ display: 'flex', gap: 6, fontSize: 14.5, fontWeight: 700 }}>
                <span
                  style={{
                    padding: '3px 9px',
                    borderRadius: 5,
                    backgroundColor: '#2563eb',
                    border:
                      overviewCard.activeBtn === 'jump'
                        ? '1.5px solid #93c5fd'
                        : '1.5px solid transparent',
                    color: '#ffffff',
                  }}
                >
                  Jump [Enter]
                </span>
                <span
                  style={{
                    padding: '3px 9px',
                    borderRadius: 5,
                    backgroundColor: overviewCard.activeBtn === 'split' ? '#0ea5e9' : '#27272a',
                    border:
                      overviewCard.activeBtn === 'split'
                        ? '1.5px solid #bae6fd'
                        : '1.5px solid transparent',
                    color: '#ffffff',
                  }}
                >
                  Split [S]
                </span>
                <span
                  style={{
                    padding: '3px 9px',
                    borderRadius: 5,
                    backgroundColor: '#3f3f46',
                    border:
                      overviewCard.activeBtn === 'close'
                        ? '1.5px solid #e4e4e7'
                        : '1.5px solid transparent',
                    color: '#ffffff',
                  }}
                >
                  x [Esc]
                </span>
              </div>
            </div>
            <div
              style={{
                fontSize: 18,
                lineHeight: 1.45,
                color: '#18181b',
                margin: 8,
                padding: 14,
                borderRadius: 8,
                backgroundColor: '#ffffff',
              }}
            >
              {overviewCard.snippet}
            </div>
          </div>
        )}

        {/* Animated Cursor Overlay */}
        {cursor && (
          <div
            style={{
              position: 'absolute',
              left: `${cursor.xPercent}%`,
              top: `${cursor.yPercent}%`,
              zIndex: 30,
              pointerEvents: 'none',
            }}
          >
            {cursor.clicking && (
              <div
                style={{
                  position: 'absolute',
                  left: -14,
                  top: -14,
                  width: 36,
                  height: 36,
                  borderRadius: 99,
                  border: '3px solid #38bdf8',
                  backgroundColor: 'rgba(56, 189, 248, 0.28)',
                }}
              />
            )}
            <svg width="30" height="30" viewBox="0 0 24 24">
              <path
                d="M4 2 L19 13 L12 14 L15 21 L12 22 L9 15 L4 19 Z"
                fill="#ffffff"
                stroke="#0f172a"
                strokeWidth="1.6"
              />
            </svg>
          </div>
        )}
      </div>
    </div>
  );
};
