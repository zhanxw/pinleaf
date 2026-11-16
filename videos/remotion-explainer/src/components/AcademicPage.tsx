import React from 'react';
import { FONT_MONO, FONT_SANS } from './StageFrame';

export type PinleafTheme = 'light' | 'dark' | 'sepia' | 'custom';

export interface HintBadge {
  code: string;
  xPercent: number;
  yPercent: number;
  active?: boolean;
  typedPrefix?: string;
}

export interface PageHighlightOverlay {
  topPx: number;
  leftPercent: number;
  widthPercent: number;
  heightPx: number;
  color: string;
  label?: string;
  kind?: 'highlight' | 'underline' | 'strikeout' | 'note';
}

export interface AcademicPageProps {
  pageNumber?: number;
  theme?: PinleafTheme;
  smartDarkPreserveFigures?: boolean;
  fitContent?: boolean;
  focusSection?: 'intro' | 'theorem' | 'figure' | 'equation' | 'slide';
  scrollOffsetPx?: number;
  highlights?: PageHighlightOverlay[];
  hints?: HintBadge[];
  readingRulerTopPx?: number | null;
  synctexHighlightTopPx?: number | null;
  areaMarquee?: { x: number; y: number; w: number; h: number } | null;
  portalMarginMarkers?: Array<{ topPx: number; label: string; active?: boolean }>;
  activeSelectionText?: boolean;
  pulseTheoremRef?: boolean;
  pulseFigureRef?: boolean;
  pulseCitationRef?: boolean;
  autoReloadVersion?: number;
}

export function getThemePalette(theme: PinleafTheme) {
  switch (theme) {
    case 'dark':
      return {
        canvasBg: '#18181b',
        pageBg: '#121318',
        textPrimary: '#e4e4e7',
        textMuted: '#a1a1aa',
        heading: '#f4f4f5',
        border: '#27272a',
        boxBg: '#1e2028',
        refColor: '#60a5fa',
      };
    case 'sepia':
      return {
        canvasBg: '#e7dec8',
        pageBg: '#f6eed9',
        textPrimary: '#3f3222',
        textMuted: '#6f5c43',
        heading: '#291e11',
        border: '#d6c6a5',
        boxBg: '#ede1c4',
        refColor: '#9a3412',
      };
    case 'custom':
      // Matches UserConfig test custom theme: bg #1e1e2e, fg #cdd6f4
      return {
        canvasBg: '#181825',
        pageBg: '#1e1e2e',
        textPrimary: '#cdd6f4',
        textMuted: '#a6adc8',
        heading: '#89b4fa',
        border: '#313244',
        boxBg: '#25263a',
        refColor: '#89dceb',
      };
    case 'light':
    default:
      return {
        canvasBg: '#e4e4e7',
        pageBg: '#ffffff',
        textPrimary: '#1e293b',
        textMuted: '#475569',
        heading: '#0f172a',
        border: '#cbd5e1',
        boxBg: '#f1f5f9',
        refColor: '#1d4ed8',
      };
  }
}

export const AcademicPage: React.FC<AcademicPageProps> = ({
  pageNumber = 1,
  theme = 'light',
  smartDarkPreserveFigures = true,
  fitContent = false,
  focusSection = 'intro',
  scrollOffsetPx = 0,
  highlights = [],
  hints = [],
  readingRulerTopPx = null,
  synctexHighlightTopPx = null,
  areaMarquee = null,
  portalMarginMarkers = [],
  activeSelectionText = false,
  pulseTheoremRef = false,
  pulseFigureRef = false,
  pulseCitationRef = false,
  autoReloadVersion = 1,
}) => {
  const pal = getThemePalette(theme);
  const darkish = theme === 'dark' || theme === 'custom';
  const invertFigure = darkish && !smartDarkPreserveFigures;

  if (focusSection === 'slide') {
    return (
      <div
        style={{
          width: '100%',
          height: '100%',
          backgroundColor: '#f8fafc',
          borderRadius: 10,
          border: '1px solid #cbd5e1',
          padding: '44px 56px',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'space-between',
          color: '#0f172a',
          boxShadow: '0 24px 60px rgba(0,0,0,0.45)',
        }}
      >
        <div>
          <div
            style={{
              fontSize: 16,
              fontWeight: 700,
              color: '#059669',
              letterSpacing: '0.08em',
              textTransform: 'uppercase',
            }}
          >
            Pinleaf Architecture Talk • Slide 4 / 12
          </div>
          <h2 style={{ fontSize: 40, fontWeight: 800, margin: '10px 0 18px', letterSpacing: '-0.02em' }}>
            Multi-View Freeze-and-Branch Reading
          </h2>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 26 }}>
            <div
              style={{
                backgroundColor: '#ecfdf5',
                border: '1.5px solid #34d399',
                borderRadius: 14,
                padding: 22,
              }}
            >
              <div style={{ fontSize: 20, fontWeight: 700, color: '#065f46', marginBottom: 8 }}>
                N-Node Reading Trail
              </div>
              <div style={{ fontSize: 16, color: '#1e293b', lineHeight: 1.55 }}>
                Freeze any source proof (#1 🔒 → #2 🔒) while routing Smart Jumps cleanly to active child pane #3.
              </div>
            </div>
            <div
              style={{
                backgroundColor: '#eff6ff',
                border: '1.5px solid #60a5fa',
                borderRadius: 14,
                padding: 22,
              }}
            >
              <div style={{ fontSize: 20, fontWeight: 700, color: '#1e40af', marginBottom: 8 }}>
                Native ISO-32000 /Annots
              </div>
              <div style={{ fontSize: 16, color: '#1e293b', lineHeight: 1.55 }}>
                26 typed highlight categories saved atomically directly into the PDF without proprietary sidecars.
              </div>
            </div>
          </div>
        </div>

        {/* Crisp mini chart */}
        <div
          style={{
            height: 150,
            borderRadius: 12,
            backgroundColor: '#f1f5f9',
            border: '1px solid #cbd5e1',
            padding: '14px 22px',
            display: 'flex',
            alignItems: 'flex-end',
            gap: 20,
          }}
        >
          {[42, 68, 84, 96, 118].map((h, i) => (
            <div key={i} style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6 }}>
              <div
                style={{
                  width: '100%',
                  height: h,
                  borderRadius: 8,
                  background: i >= 3 ? 'linear-gradient(180deg, #10b981, #059669)' : '#64748b',
                }}
              />
              <span style={{ fontSize: 13, fontWeight: 600, color: '#475569' }}>
                {['Preview', 'Okular', 'Skim', 'Sioyek', 'Pinleaf'][i]}
              </span>
            </div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        position: 'relative',
        width: fitContent ? '98%' : '90%',
        height: '100%',
        margin: '0 auto',
        backgroundColor: pal.pageBg,
        color: pal.textPrimary,
        borderRadius: 6,
        border: `1px solid ${pal.border}`,
        boxShadow: '0 8px 28px rgba(0, 0, 0, 0.16)',
        overflow: 'hidden',
        padding: fitContent
          ? '18px 18px'
          : portalMarginMarkers.length > 0
          ? '22px 30px 22px 46px'
          : '22px 34px',
        fontFamily: '"Georgia", "Times New Roman", serif',
      }}
    >
      {/* Portal Left Margin Indicators */}
      {portalMarginMarkers.map((m, idx) => (
        <div
          key={idx}
          style={{
            position: 'absolute',
            left: 6,
            top: m.topPx,
            display: 'flex',
            alignItems: 'center',
            gap: 3,
            zIndex: 4,
          }}
        >
          <div
            style={{
              width: 5,
              height: 26,
              borderRadius: 999,
              backgroundColor: m.active ? '#10b981' : '#64748b',
              boxShadow: m.active ? '0 0 10px rgba(16,185,129,0.7)' : 'none',
            }}
          />
          <span
            style={{
              fontFamily: FONT_MONO,
              fontSize: 9.5,
              fontWeight: 800,
              color: m.active ? '#059669' : pal.textMuted,
              backgroundColor: m.active ? 'rgba(16,185,129,0.16)' : 'rgba(148,163,184,0.12)',
              padding: '1px 4px',
              borderRadius: 4,
            }}
          >
            {m.label.split('→')[0]}
          </span>
        </div>
      ))}

      {/* Scrollable Inner PDF Content */}
      <div
        style={{
          position: 'relative',
          transform: `translateY(${-scrollOffsetPx}px)`,
        }}
      >
        {/* Top Paper Header */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            fontFamily: FONT_SANS,
            fontSize: 11,
            color: pal.textMuted,
            borderBottom: `1px solid ${pal.border}`,
            paddingBottom: 6,
            marginBottom: 12,
          }}
        >
          <span>Scalable Multi-View Document Architectures • Rev {autoReloadVersion}.0</span>
          <span>Page {pageNumber} of 12</span>
        </div>

        <div
          style={{
            fontFamily: FONT_SANS,
            fontSize: 21,
            fontWeight: 700,
            color: pal.heading,
            lineHeight: 1.24,
            marginBottom: 6,
          }}
        >
          {focusSection === 'theorem'
            ? '3. Theoretical Bounds & Theorem Proofs'
            : focusSection === 'figure'
            ? '4. Empirical Latency & Memory Benchmarks'
            : focusSection === 'equation'
            ? '5. Variational Scaling Formulation'
            : 'Attention-Preserving Split Graphs for Academic PDFs'}
        </div>

        <div
          style={{
            fontFamily: FONT_SANS,
            fontSize: 12,
            color: pal.textMuted,
            marginBottom: 14,
          }}
        >
          E. Vance, M. Tanaka • Dept. of Computer Science • DOI:{' '}
          <span style={{ color: pal.refColor }}>10.1145/3597503.3623310</span>
        </div>

        {/* Two-column layout matching academic papers */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 22, fontSize: 13.5, lineHeight: 1.54 }}>
          {/* Column 1 */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <p style={{ margin: 0 }}>
              <strong style={{ fontFamily: FONT_SANS, color: pal.heading }}>1. Introduction.</strong>{' '}
              Reading technical manuscripts rarely proceeds strictly top-to-bottom. Readers inspect{' '}
              <span
                style={{
                  color: pal.refColor,
                  fontWeight: 700,
                  backgroundColor: pulseTheoremRef ? 'rgba(59, 130, 246, 0.22)' : 'transparent',
                  padding: '0 3px',
                  borderRadius: 4,
                  outline: pulseTheoremRef ? '2px solid #3b82f6' : 'none',
                }}
              >
                Theorem 2
              </span>{' '}
              while verifying empirical ablation plots in{' '}
              <span
                style={{
                  color: pal.refColor,
                  fontWeight: 700,
                  backgroundColor: pulseFigureRef ? 'rgba(16, 185, 129, 0.24)' : 'transparent',
                  padding: '0 3px',
                  borderRadius: 4,
                  outline: pulseFigureRef ? '2px solid #10b981' : 'none',
                }}
              >
                Figure 1
              </span>
              .
            </p>

            <div
              style={{
                backgroundColor: pal.boxBg,
                borderLeft: '3px solid #3b82f6',
                padding: '9px 11px',
                borderRadius: '0 6px 6px 0',
                fontSize: 13,
              }}
            >
              <strong style={{ fontFamily: FONT_SANS, color: pal.heading }}>Theorem 1 (Anchored Stability).</strong>{' '}
              Given a frozen source anchor <span style={{ fontFamily: FONT_MONO }}>a₀</span>, smart-routing
              child transitions preserves context entropy across <span style={{ fontFamily: FONT_MONO }}>k</span>{' '}
              branches{' '}
              <span
                style={{
                  backgroundColor: activeSelectionText ? 'rgba(250, 204, 21, 0.45)' : 'transparent',
                  borderRadius: 3,
                  padding: '0 2px',
                }}
              >
                without losing spatial recall
              </span>
              .
            </div>

            <p style={{ margin: 0 }}>
              Prior studies on document navigation{' '}
              <span
                style={{
                  color: pal.refColor,
                  fontWeight: 700,
                  backgroundColor: pulseCitationRef ? 'rgba(139, 92, 246, 0.22)' : 'transparent',
                  padding: '0 3px',
                  borderRadius: 4,
                }}
              >
                [12]
              </span>{' '}
              demonstrate that single-pane scrolling induces up to 3.4x higher cognitive switch cost than pinned
              reference split views and paired portals.
            </p>

            <p style={{ margin: 0 }}>
              Under Eq. (4), tile cache eviction stays bounded by <span style={{ fontFamily: FONT_MONO }}>O(V)</span>{' '}
              for active panes when zoom scales lock independently per reading node.
            </p>
          </div>

          {/* Column 2 */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            {/* Figure card (demonstrates Smart Dark Mode Figure Preservation) */}
            <div
              style={{
                borderRadius: 8,
                border: `1px solid ${pal.border}`,
                padding: 10,
                backgroundColor: invertFigure ? '#0f172a' : '#ffffff',
                color: invertFigure ? '#f8fafc' : '#0f172a',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  fontFamily: FONT_SANS,
                  fontSize: 11,
                  fontWeight: 700,
                  marginBottom: 8,
                }}
              >
                <span>
                  {focusSection === 'equation'
                    ? 'Equation (4) • Convergence Rate'
                    : 'Figure 1 • Branch Switch Cost (ms)'}
                </span>
                <span style={{ color: invertFigure ? '#f87171' : '#059669' }}>
                  {invertFigure ? 'Inverted Bitmap' : 'True Color Preserved'}
                </span>
              </div>

              {focusSection === 'equation' ? (
                <div
                  style={{
                    height: 92,
                    borderRadius: 6,
                    backgroundColor: '#eff6ff',
                    border: '1px solid #bfdbfe',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontFamily: FONT_MONO,
                    fontSize: 16,
                    fontWeight: 700,
                    color: '#1e3a8a',
                  }}
                >
                  L(θ) = Σᵢ wᵢ · ‖xᵢ − P(aᵢ)‖² + λ‖∇θ‖
                </div>
              ) : (
                <div
                  style={{
                    height: 92,
                    borderRadius: 6,
                    backgroundColor: invertFigure ? '#111827' : '#f8fafc',
                    border: invertFigure ? '1px solid #374151' : '1px solid #e2e8f0',
                    padding: '8px 12px',
                    display: 'flex',
                    alignItems: 'flex-end',
                    gap: 10,
                  }}
                >
                  {[
                    { h: 68, c: invertFigure ? '#06b6d4' : '#ef4444', l: 'Scroll' },
                    { h: 52, c: invertFigure ? '#a855f7' : '#f59e0b', l: 'Tabs' },
                    { h: 38, c: invertFigure ? '#f43f5e' : '#3b82f6', l: 'Split' },
                    { h: 20, c: invertFigure ? '#f472b6' : '#10b981', l: 'Portal' },
                  ].map((bar, i) => (
                    <div
                      key={i}
                      style={{
                        flex: 1,
                        display: 'flex',
                        flexDirection: 'column',
                        alignItems: 'center',
                        gap: 4,
                      }}
                    >
                      <div
                        style={{
                          width: '100%',
                          height: bar.h,
                          borderRadius: 4,
                          backgroundColor: bar.c,
                        }}
                      />
                      <span
                        style={{
                          fontFamily: FONT_SANS,
                          fontSize: 10,
                          fontWeight: 600,
                          color: invertFigure ? '#cbd5e1' : '#334155',
                        }}
                      >
                        {bar.l}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <p style={{ margin: 0 }}>
              <strong style={{ fontFamily: FONT_SANS, color: pal.heading }}>Theorem 2 (Incremental Sync).</strong>{' '}
              Wait-free page rasterization updates annotations and SyncTeX highlights without re-decoding streams.{' '}
              {autoReloadVersion > 1 && (
                <span style={{ color: '#059669', fontWeight: 700 }}>
                  [Updated in compile #{autoReloadVersion}: added Lemma 3 proof bounds.]
                </span>
              )}
            </p>

            <p style={{ margin: 0 }}>
              All user highlights, notes, and category tags persist inside standard ISO-32000 PDF dictionaries so
              external viewers preserve fidelity seamlessly.
            </p>
          </div>
        </div>
      </div>

      {/* Overlays: Highlights / Underlines / Strikeouts */}
      {highlights.map((h, i) => (
        <div
          key={i}
          style={{
            position: 'absolute',
            top: h.topPx,
            left: `${h.leftPercent}%`,
            width: `${h.widthPercent}%`,
            height: h.kind === 'underline' ? 3 : h.heightPx,
            marginTop: h.kind === 'underline' ? h.heightPx - 2 : 0,
            backgroundColor:
              h.kind === 'underline'
                ? h.color
                : h.kind === 'strikeout'
                ? 'transparent'
                : `${h.color}55`,
            borderBottom: h.kind === 'strikeout' ? `2.5px solid ${h.color}` : 'none',
            borderRadius: 3,
            pointerEvents: 'none',
          }}
        >
          {h.label && (
            <span
              style={{
                position: 'absolute',
                top: -17,
                right: 0,
                fontFamily: FONT_MONO,
                fontSize: 10,
                fontWeight: 800,
                padding: '1px 6px',
                borderRadius: 4,
                backgroundColor: h.color,
                color: '#0f172a',
              }}
            >
              {h.label}
            </span>
          )}
        </div>
      ))}

      {/* Area Marquee Overlay */}
      {areaMarquee && (
        <div
          style={{
            position: 'absolute',
            left: `${areaMarquee.x}%`,
            top: areaMarquee.y,
            width: `${areaMarquee.w}%`,
            height: areaMarquee.h,
            border: '2px dashed #f59e0b',
            backgroundColor: 'rgba(245, 158, 11, 0.14)',
            borderRadius: 6,
          }}
        />
      )}

      {/* Visual Reading Ruler */}
      {readingRulerTopPx !== null && (
        <div
          style={{
            position: 'absolute',
            left: 14,
            right: 14,
            top: readingRulerTopPx,
            height: 24,
            backgroundColor: 'rgba(56, 189, 248, 0.16)',
            borderTop: '1.5px solid rgba(56, 189, 248, 0.75)',
            borderBottom: '1.5px solid rgba(56, 189, 248, 0.75)',
            borderRadius: 4,
          }}
        />
      )}

      {/* SyncTeX Forward Cyan Highlight */}
      {synctexHighlightTopPx !== null && (
        <div
          style={{
            position: 'absolute',
            left: '52%',
            right: 26,
            top: synctexHighlightTopPx,
            height: 22,
            backgroundColor: 'rgba(34, 211, 238, 0.34)',
            border: '1.5px solid #06b6d4',
            borderRadius: 4,
            boxShadow: '0 0 18px rgba(6, 182, 212, 0.45)',
          }}
        />
      )}

      {/* Vimium 2-Letter Hint Badges */}
      {hints.map((hint, idx) => (
        <div
          key={idx}
          style={{
            position: 'absolute',
            left: `${hint.xPercent}%`,
            top: `${hint.yPercent}%`,
            padding: '2px 7px',
            borderRadius: 5,
            backgroundColor: hint.active ? '#f97316' : '#facc15',
            border: '1.5px solid #422006',
            color: '#0f172a',
            fontFamily: FONT_MONO,
            fontSize: 13,
            fontWeight: 800,
            boxShadow: '0 4px 10px rgba(0,0,0,0.35)',
            zIndex: 10,
          }}
        >
          {hint.typedPrefix ? (
            <>
              <span style={{ opacity: 0.38 }}>{hint.typedPrefix}</span>
              <span>{hint.code.slice(hint.typedPrefix.length)}</span>
            </>
          ) : (
            hint.code
          )}
        </div>
      ))}
    </div>
  );
};
