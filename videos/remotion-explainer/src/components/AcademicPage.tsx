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
        textPrimary: '#0f172a',
        textMuted: '#475569',
        heading: '#020617',
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
          borderRadius: 12,
          border: '1px solid #cbd5e1',
          padding: '38px 54px',
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
              fontSize: 20,
              fontWeight: 800,
              color: '#059669',
              letterSpacing: '0.08em',
              textTransform: 'uppercase',
            }}
          >
            Pinleaf Architecture Talk • Slide 4 / 12
          </div>
          <h2 style={{ fontSize: 46, fontWeight: 800, margin: '8px 0 18px', letterSpacing: '-0.02em' }}>
            Multi-View Freeze-and-Branch Reading
          </h2>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 26 }}>
            <div
              style={{
                backgroundColor: '#ecfdf5',
                border: '2px solid #34d399',
                borderRadius: 14,
                padding: 22,
              }}
            >
              <div style={{ fontSize: 25, fontWeight: 800, color: '#065f46', marginBottom: 8 }}>
                N-Node Reading Trail
              </div>
              <div style={{ fontSize: 20, color: '#1e293b', lineHeight: 1.45 }}>
                Freeze source proofs (#1 🔒 → #2 🔒) while routing Smart Jumps cleanly to pane #3.
              </div>
            </div>
            <div
              style={{
                backgroundColor: '#eff6ff',
                border: '2px solid #60a5fa',
                borderRadius: 14,
                padding: 22,
              }}
            >
              <div style={{ fontSize: 25, fontWeight: 800, color: '#1e40af', marginBottom: 8 }}>
                Native ISO-32000 /Annots
              </div>
              <div style={{ fontSize: 20, color: '#1e293b', lineHeight: 1.45 }}>
                26 typed highlight buckets saved atomically inside the PDF without proprietary sidecars.
              </div>
            </div>
          </div>
        </div>

        {/* Crisp mini chart */}
        <div
          style={{
            height: 175,
            borderRadius: 12,
            backgroundColor: '#f1f5f9',
            border: '1px solid #cbd5e1',
            padding: '16px 24px',
            display: 'flex',
            alignItems: 'flex-end',
            gap: 22,
          }}
        >
          {[52, 78, 96, 112, 134].map((h, i) => (
            <div key={i} style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6 }}>
              <div
                style={{
                  width: '100%',
                  height: h,
                  borderRadius: 8,
                  background: i >= 3 ? 'linear-gradient(180deg, #10b981, #059669)' : '#64748b',
                }}
              />
              <span style={{ fontSize: 18, fontWeight: 700, color: '#334155' }}>
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
        width: fitContent ? '98%' : '93%',
        height: '100%',
        margin: '0 auto',
        backgroundColor: pal.pageBg,
        color: pal.textPrimary,
        borderRadius: 6,
        border: `1px solid ${pal.border}`,
        boxShadow: '0 8px 28px rgba(0, 0, 0, 0.16)',
        overflow: 'hidden',
        padding: fitContent
          ? '18px 20px'
          : portalMarginMarkers.length > 0
          ? '20px 26px 20px 66px'
          : '20px 30px',
        fontFamily: '"Georgia", "Times New Roman", serif',
      }}
    >
      {/* Portal Left Margin Indicators */}
      {portalMarginMarkers.map((m, idx) => (
        <div
          key={idx}
          style={{
            position: 'absolute',
            left: 8,
            top: m.topPx,
            display: 'flex',
            alignItems: 'center',
            gap: 4,
            zIndex: 4,
          }}
        >
          <div
            style={{
              width: 6,
              height: 32,
              borderRadius: 999,
              backgroundColor: m.active ? '#10b981' : '#64748b',
              boxShadow: m.active ? '0 0 12px rgba(16,185,129,0.75)' : 'none',
            }}
          />
          <span
            style={{
              fontFamily: FONT_MONO,
              fontSize: 15.5,
              fontWeight: 800,
              color: m.active ? '#059669' : pal.textMuted,
              backgroundColor: m.active ? 'rgba(16,185,129,0.18)' : 'rgba(148,163,184,0.14)',
              padding: '3px 7px',
              borderRadius: 5,
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
            fontSize: 15.5,
            fontWeight: 600,
            color: pal.textMuted,
            borderBottom: `1px solid ${pal.border}`,
            paddingBottom: 6,
            marginBottom: 10,
          }}
        >
          <span>Scalable Multi-View PDF Architectures • Rev {autoReloadVersion}.0</span>
          <span>Page {pageNumber} of 12</span>
        </div>

        <div
          style={{
            fontFamily: FONT_SANS,
            fontSize: 28,
            fontWeight: 800,
            color: pal.heading,
            lineHeight: 1.2,
            marginBottom: 6,
            letterSpacing: '-0.02em',
          }}
        >
          {focusSection === 'theorem'
            ? '3. Theoretical Bounds & Theorem Proofs'
            : focusSection === 'figure'
            ? '4. Empirical Latency & Memory Benchmarks'
            : focusSection === 'equation'
            ? '5. Variational Scaling Formulation'
            : 'Attention-Preserving Split Graphs for PDFs'}
        </div>

        <div
          style={{
            fontFamily: FONT_SANS,
            fontSize: 16.5,
            fontWeight: 600,
            color: pal.textMuted,
            marginBottom: 14,
          }}
        >
          E. Vance, M. Tanaka • DOI:{' '}
          <span style={{ color: pal.refColor }}>10.1145/3597503.3623310</span>
        </div>

        {/* Concise two-column layout scaled for clear GIF legibility */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: '1fr 1fr',
            gap: 22,
            fontSize: 18.5,
            lineHeight: 1.44,
          }}
        >
          {/* Column 1 */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <p style={{ margin: 0 }}>
              <strong style={{ fontFamily: FONT_SANS, color: pal.heading }}>1. Intro.</strong>{' '}
              Readers inspect{' '}
              <span
                style={{
                  color: pal.refColor,
                  fontWeight: 800,
                  backgroundColor: pulseTheoremRef ? 'rgba(59, 130, 246, 0.24)' : 'transparent',
                  padding: '0 4px',
                  borderRadius: 4,
                  outline: pulseTheoremRef ? '2.5px solid #3b82f6' : 'none',
                }}
              >
                Theorem 2
              </span>{' '}
              while checking empirical plots in{' '}
              <span
                style={{
                  color: pal.refColor,
                  fontWeight: 800,
                  backgroundColor: pulseFigureRef ? 'rgba(16, 185, 129, 0.26)' : 'transparent',
                  padding: '0 4px',
                  borderRadius: 4,
                  outline: pulseFigureRef ? '2.5px solid #10b981' : 'none',
                }}
              >
                Figure 1
              </span>
              .
            </p>

            <div
              style={{
                backgroundColor: pal.boxBg,
                borderLeft: '5px solid #3b82f6',
                padding: '11px 14px',
                borderRadius: '0 8px 8px 0',
                fontSize: 18,
              }}
            >
              <strong style={{ fontFamily: FONT_SANS, color: pal.heading }}>Theorem 1 (Anchored).</strong>{' '}
              Frozen anchor <span style={{ fontFamily: FONT_MONO }}>a₀</span> routes branches{' '}
              <span
                style={{
                  backgroundColor: activeSelectionText ? 'rgba(250, 204, 21, 0.5)' : 'transparent',
                  borderRadius: 4,
                  padding: '0 3px',
                }}
              >
                without losing spatial recall
              </span>
              .
            </div>

            <p style={{ margin: 0 }}>
              Prior work{' '}
              <span
                style={{
                  color: pal.refColor,
                  fontWeight: 800,
                  backgroundColor: pulseCitationRef ? 'rgba(139, 92, 246, 0.24)' : 'transparent',
                  padding: '0 4px',
                  borderRadius: 4,
                }}
              >
                [12]
              </span>{' '}
              shows split views cut switch cost by 3.4× under Eq. (4) bounds.
            </p>
          </div>

          {/* Column 2 */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            {/* Figure card */}
            <div
              style={{
                borderRadius: 10,
                border: `1.5px solid ${pal.border}`,
                padding: 12,
                backgroundColor: invertFigure ? '#0f172a' : '#ffffff',
                color: invertFigure ? '#f8fafc' : '#0f172a',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  fontFamily: FONT_SANS,
                  fontSize: 15.5,
                  fontWeight: 800,
                  marginBottom: 8,
                }}
              >
                <span>
                  {focusSection === 'equation' ? 'Equation (4)' : 'Figure 1 • Switch Cost'}
                </span>
                <span style={{ color: invertFigure ? '#f87171' : '#059669' }}>
                  {invertFigure ? 'Inverted' : 'True Color'}
                </span>
              </div>

              {focusSection === 'equation' ? (
                <div
                  style={{
                    height: 112,
                    borderRadius: 8,
                    backgroundColor: '#eff6ff',
                    border: '1px solid #bfdbfe',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontFamily: FONT_MONO,
                    fontSize: 21,
                    fontWeight: 800,
                    color: '#1e3a8a',
                  }}
                >
                  L(θ) = Σᵢ wᵢ‖xᵢ − P(aᵢ)‖² + λ‖∇θ‖
                </div>
              ) : (
                <div
                  style={{
                    height: 118,
                    borderRadius: 8,
                    backgroundColor: invertFigure ? '#111827' : '#f8fafc',
                    border: invertFigure ? '1px solid #374151' : '1px solid #e2e8f0',
                    padding: '10px 14px',
                    display: 'flex',
                    alignItems: 'flex-end',
                    gap: 12,
                  }}
                >
                  {[
                    { h: 78, c: invertFigure ? '#06b6d4' : '#ef4444', l: 'Scroll' },
                    { h: 60, c: invertFigure ? '#a855f7' : '#f59e0b', l: 'Tabs' },
                    { h: 42, c: invertFigure ? '#f43f5e' : '#3b82f6', l: 'Split' },
                    { h: 24, c: invertFigure ? '#f472b6' : '#10b981', l: 'Portal' },
                  ].map((bar, i) => (
                    <div
                      key={i}
                      style={{
                        flex: 1,
                        display: 'flex',
                        flexDirection: 'column',
                        alignItems: 'center',
                        gap: 5,
                      }}
                    >
                      <div
                        style={{
                          width: '100%',
                          height: bar.h,
                          borderRadius: 5,
                          backgroundColor: bar.c,
                        }}
                      />
                      <span
                        style={{
                          fontFamily: FONT_SANS,
                          fontSize: 15,
                          fontWeight: 700,
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
              Wait-free page tiles update highlights and SyncTeX spans in place.{' '}
              {autoReloadVersion > 1 && (
                <span style={{ color: '#059669', fontWeight: 800 }}>
                  [Updated #{autoReloadVersion}: Lemma 3 added.]
                </span>
              )}
            </p>

            <p style={{ margin: 0 }}>
              Annotations persist inside standard ISO-32000 PDF{' '}
              <span style={{ fontFamily: FONT_MONO }}>/Annots</span> dictionaries.
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
            height: h.kind === 'underline' ? 4 : h.heightPx,
            marginTop: h.kind === 'underline' ? h.heightPx - 2 : 0,
            backgroundColor:
              h.kind === 'underline'
                ? h.color
                : h.kind === 'strikeout'
                ? 'transparent'
                : `${h.color}55`,
            borderBottom: h.kind === 'strikeout' ? `3.5px solid ${h.color}` : 'none',
            borderRadius: 4,
            pointerEvents: 'none',
          }}
        >
          {h.label && (
            <span
              style={{
                position: 'absolute',
                top: -24,
                right: 0,
                fontFamily: FONT_MONO,
                fontSize: 15.5,
                fontWeight: 800,
                padding: '2px 8px',
                borderRadius: 5,
                backgroundColor: h.color,
                color: '#0f172a',
                boxShadow: '0 2px 6px rgba(0,0,0,0.2)',
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
            border: '3px dashed #f59e0b',
            backgroundColor: 'rgba(245, 158, 11, 0.16)',
            borderRadius: 8,
          }}
        />
      )}

      {/* Visual Reading Ruler */}
      {readingRulerTopPx !== null && (
        <div
          style={{
            position: 'absolute',
            left: 16,
            right: 16,
            top: readingRulerTopPx,
            height: 30,
            backgroundColor: 'rgba(56, 189, 248, 0.18)',
            borderTop: '2px solid rgba(56, 189, 248, 0.85)',
            borderBottom: '2px solid rgba(56, 189, 248, 0.85)',
            borderRadius: 5,
          }}
        />
      )}

      {/* SyncTeX Forward Cyan Highlight */}
      {synctexHighlightTopPx !== null && (
        <div
          style={{
            position: 'absolute',
            left: '51%',
            right: 26,
            top: synctexHighlightTopPx,
            height: 28,
            backgroundColor: 'rgba(34, 211, 238, 0.36)',
            border: '2px solid #06b6d4',
            borderRadius: 5,
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
            padding: '4px 11px',
            borderRadius: 7,
            backgroundColor: hint.active ? '#f97316' : '#facc15',
            border: '2px solid #422006',
            color: '#0f172a',
            fontFamily: FONT_MONO,
            fontSize: 22,
            fontWeight: 800,
            boxShadow: '0 6px 14px rgba(0,0,0,0.38)',
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
