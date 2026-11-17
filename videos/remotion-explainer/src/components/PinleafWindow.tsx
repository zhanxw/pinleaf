import React from 'react';
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
  presentationMode?: boolean;
  splitAxis?: 'horizontal' | 'vertical';
  trailNodes?: TrailPillSpec[];
  panes: PaneSpec[];
  zoomLabel?: string;
  autoReloadEnabled?: boolean;
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

export const PinleafWindow: React.FC<PinleafWindowProps> = ({
  title = 'Pinleaf — Attention_Preserving_Split_Graphs.pdf',
  theme = 'light',
  presentationMode = false,
  splitAxis = 'horizontal',
  trailNodes,
  panes,
  zoomLabel = '118%',
  autoReloadEnabled = false,
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
          height: 40,
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
            fontSize: 18,
            fontWeight: 700,
            color: darkChrome ? '#e2e8f0' : '#334155',
          }}
        >
          {title}
        </div>
        <div style={{ width: 56 }} />
      </div>

      {/* Pinleaf Toolbar */}
      <div
        style={{
          height: 50,
          backgroundColor: darkChrome ? '#111827' : '#ffffff',
          borderBottom: `1px solid ${pal.border}`,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 16px',
          fontSize: 17,
          fontWeight: 700,
          flexShrink: 0,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}>
          {['Sidebar', 'Search', 'TOC', 'Split ⌘D'].map((btn) => (
            <div
              key={btn}
              style={{
                padding: '6px 12px',
                borderRadius: 8,
                backgroundColor: darkChrome ? '#1f2937' : '#f1f5f9',
                color: darkChrome ? '#e2e8f0' : '#1e293b',
                border: `1px solid ${pal.border}`,
              }}
            >
              {btn}
            </div>
          ))}
          {trailNodes && trailNodes.length > 0 && (
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 7,
                marginLeft: 8,
                padding: '4px 10px',
                borderRadius: 9,
                backgroundColor: darkChrome ? '#0f172a' : '#f8fafc',
                border: `1px solid ${pal.border}`,
              }}
            >
              {trailNodes.map((n, idx) => (
                <React.Fragment key={n.id}>
                  {idx > 0 && <span style={{ color: '#64748b' }}>→</span>}
                  <span
                    style={{
                      padding: '3px 10px',
                      borderRadius: 7,
                      fontFamily: FONT_MONO,
                      fontSize: 16,
                      fontWeight: 800,
                      backgroundColor: n.frozen
                        ? 'rgba(245, 158, 11, 0.2)'
                        : n.active
                        ? 'rgba(16, 185, 129, 0.22)'
                        : 'rgba(148, 163, 184, 0.16)',
                      color: n.frozen ? '#d97706' : n.active ? '#059669' : '#64748b',
                      border: n.frozen
                        ? '1.5px solid rgba(245, 158, 11, 0.5)'
                        : n.active
                        ? '1.5px solid rgba(16, 185, 129, 0.5)'
                        : '1px solid transparent',
                    }}
                  >
                    #{n.id} {n.frozen ? '🔒 Frozen' : n.active ? '● Active' : ''}
                  </span>
                </React.Fragment>
              ))}
            </div>
          )}
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          {autoReloadEnabled && (
            <div
              style={{
                padding: '5px 12px',
                borderRadius: 8,
                backgroundColor: 'rgba(34, 211, 238, 0.18)',
                border: '1.5px solid rgba(34, 211, 238, 0.55)',
                color: darkChrome ? '#67e8f9' : '#0e7490',
                fontFamily: FONT_MONO,
                fontSize: 16,
              }}
            >
              ⟳ Auto-Reload ON
            </div>
          )}
          <div
            style={{
              padding: '5px 12px',
              borderRadius: 8,
              backgroundColor: darkChrome ? '#1f2937' : '#f1f5f9',
              color: darkChrome ? '#e2e8f0' : '#1e293b',
              fontFamily: FONT_MONO,
            }}
          >
            Theme: {theme.toUpperCase()}
          </div>
          <div
            style={{
              padding: '5px 12px',
              borderRadius: 8,
              backgroundColor: darkChrome ? '#1f2937' : '#f1f5f9',
              color: darkChrome ? '#e2e8f0' : '#1e293b',
              fontFamily: FONT_MONO,
            }}
          >
            {zoomLabel}
          </div>
          <div
            style={{
              padding: '6px 14px',
              borderRadius: 8,
              backgroundColor: '#10b981',
              color: '#064e3b',
              fontWeight: 800,
            }}
          >
            Save ⌘S
          </div>
        </div>
      </div>

      {/* Main Content Body */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden', position: 'relative' }}>
        {sidebar && (
          <div
            style={{
              width: 460,
              backgroundColor: darkChrome ? '#0f172a' : '#f8fafc',
              borderRight: `1px solid ${pal.border}`,
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
                  <div style={{ fontSize: 17, color: pal.textPrimary, lineHeight: 1.35 }}>
                    “{item.quote}”
                  </div>
                  {item.note && (
                    <div
                      style={{
                        marginTop: 5,
                        fontSize: 15.5,
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

        {/* Panes Container */}
        <div
          style={{
            flex: 1,
            display: 'flex',
            flexDirection: splitAxis === 'vertical' ? 'column' : 'row',
            gap: 12,
            padding: 12,
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
                borderRadius: 11,
                border: pane.active
                  ? '2.5px solid #10b981'
                  : pane.frozen
                  ? '2.5px solid #f59e0b'
                  : pane.portalFollower
                  ? '2.5px solid #38bdf8'
                  : `1px solid ${pal.border}`,
                backgroundColor: pal.canvasBg,
                overflow: 'hidden',
              }}
            >
              {/* Pane mini-header */}
              <div
                style={{
                  height: 38,
                  backgroundColor: darkChrome ? '#0f172a' : '#e2e8f0',
                  padding: '0 12px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  fontSize: 16.5,
                  fontWeight: 800,
                  flexShrink: 0,
                }}
              >
                <span style={{ color: pal.heading }}>{pane.label}</span>
                <div style={{ display: 'flex', gap: 8 }}>
                  {pane.frozen && (
                    <span
                      style={{
                        padding: '2px 9px',
                        borderRadius: 999,
                        backgroundColor: 'rgba(245, 158, 11, 0.24)',
                        color: '#b45309',
                        fontSize: 15,
                      }}
                    >
                      🔒 FROZEN
                    </span>
                  )}
                  {pane.portalFollower && (
                    <span
                      style={{
                        padding: '2px 10px',
                        borderRadius: 999,
                        backgroundColor: 'rgba(56, 189, 248, 0.25)',
                        color: darkChrome ? '#7dd3fc' : '#0369a1',
                        fontSize: 15.5,
                      }}
                    >
                      ⇄ Portal Follower{pane.activePortalTarget ? ` • ${pane.activePortalTarget}` : ''}
                    </span>
                  )}
                  {pane.active && !pane.frozen && !pane.portalFollower && (
                    <span
                      style={{
                        padding: '2px 9px',
                        borderRadius: 999,
                        backgroundColor: 'rgba(16, 185, 129, 0.24)',
                        color: '#047857',
                        fontSize: 15,
                      }}
                    >
                      ACTIVE
                    </span>
                  )}
                </div>
              </div>

              <div style={{ flex: 1, padding: '8px 6px', overflow: 'hidden' }}>
                <AcademicPage {...pane.pageProps} theme={pane.pageProps.theme ?? theme} />
              </div>
            </div>
          ))}
        </div>

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
              backgroundColor: 'rgba(15, 23, 42, 0.97)',
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
                backgroundColor: '#020617',
                border: '1.5px solid #334155',
                marginBottom: 14,
                fontFamily: FONT_MONO,
                fontSize: 23,
                fontWeight: 700,
              }}
            >
              <span>
                <span style={{ color: '#60a5fa', fontWeight: 800 }}>/ </span>
                {commandPalette.query}
                <span style={{ color: '#60a5fa' }}>|</span>
              </span>
              <span style={{ fontSize: 16, color: '#94a3b8' }}>Esc to close</span>
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
                        fontSize: 19,
                        fontWeight: 800,
                        padding: '4px 12px',
                        borderRadius: 7,
                        backgroundColor: '#1e293b',
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

        {/* Floating Overview Card */}
        {overviewCard && (
          <div
            style={{
              position: 'absolute',
              left: `${overviewCard.xPercent}%`,
              top: `${overviewCard.yPercent}%`,
              width: 560,
              transform: `scale(${overviewCard.scale ?? 1})`,
              borderRadius: 14,
              backgroundColor: 'rgba(15, 23, 42, 0.97)',
              border: '2px solid #60a5fa',
              boxShadow: '0 22px 54px rgba(0,0,0,0.62)',
              padding: 18,
              color: '#f8fafc',
              zIndex: 15,
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                marginBottom: 10,
              }}
            >
              <span style={{ fontSize: 19, fontWeight: 800, color: '#93c5fd' }}>
                Overview • {overviewCard.title}
              </span>
              <span
                style={{
                  fontFamily: FONT_MONO,
                  fontSize: 16,
                  fontWeight: 700,
                  padding: '3px 9px',
                  borderRadius: 6,
                  backgroundColor: '#1e293b',
                }}
              >
                {overviewCard.pageLabel}
              </span>
            </div>
            <div
              style={{
                fontSize: 18.5,
                lineHeight: 1.45,
                color: '#f1f5f9',
                padding: 12,
                borderRadius: 9,
                backgroundColor: '#020617',
                border: '1px solid #334155',
              }}
            >
              {overviewCard.snippet}
            </div>
          </div>
        )}

        {/* KeyPress Callout + Status Banner */}
        {keyPressChip && (
          <div
            style={{
              position: 'absolute',
              bottom: 16,
              left: sidebar ? 478 : 18,
              padding: '9px 16px',
              borderRadius: 11,
              backgroundColor: 'rgba(2, 6, 23, 0.94)',
              border: '2px solid #34d399',
              fontFamily: FONT_MONO,
              fontSize: 20,
              fontWeight: 800,
              color: '#34d399',
              boxShadow: '0 8px 24px rgba(0,0,0,0.45)',
              zIndex: 20,
            }}
          >
            {keyPressChip}
          </div>
        )}

        {statusBanner && (
          <div
            style={{
              position: 'absolute',
              bottom: 16,
              right: 18,
              padding: '9px 16px',
              borderRadius: 11,
              backgroundColor:
                statusBanner.tone === 'ok'
                  ? 'rgba(6, 78, 59, 0.96)'
                  : 'rgba(15, 23, 42, 0.96)',
              border:
                statusBanner.tone === 'ok'
                  ? '2px solid #34d399'
                  : '2px solid rgba(148,163,184,0.5)',
              color: '#f8fafc',
              fontFamily: FONT_MONO,
              fontSize: 18.5,
              fontWeight: 700,
              boxShadow: '0 8px 26px rgba(0,0,0,0.45)',
              zIndex: 20,
            }}
          >
            {statusBanner.text}
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
