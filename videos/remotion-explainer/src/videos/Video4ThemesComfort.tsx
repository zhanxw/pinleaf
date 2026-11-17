import React from 'react';
import { interpolate, useCurrentFrame, useVideoConfig } from 'remotion';
import timingsJson from '../generated/timings.json';
import type { TimingsManifest } from '../types';
import type { PinleafTheme } from '../components/AcademicPage';
import { FONT_MONO, FONT_SANS } from '../components/StageFrame';
import { PinleafWindow } from '../components/PinleafWindow';
import { VideoSequenceShell } from '../components/VideoSequenceShell';

const manifest = timingsJson as TimingsManifest;
const VIDEO = manifest.videos.ThemesComfortPresentation;

const Scene1ThemeCycle: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const themes: Array<{ key: PinleafTheme; label: string; desc: string }> = [
    { key: 'light', label: 'LIGHT', desc: 'Crisp Paper White' },
    { key: 'dark', label: 'DARK', desc: 'Midnight OLED Invert' },
    { key: 'sepia', label: 'SEPIA', desc: 'Warm Paper Comfort' },
    { key: 'custom', label: 'CUSTOM', desc: 'Catppuccin #1e1e2e / #cdd6f4' },
  ];
  const idx = Math.min(
    3,
    Math.floor(
      interpolate(frame, [0, durationInFrames], [0, 4], {
        extrapolateLeft: 'clamp',
        extrapolateRight: 'clamp',
      }),
    ),
  );
  const current = themes[idx];

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14, alignItems: 'center' }}>
      <PinleafWindow
        heightPx={778}
        theme={current.key}
        keyPressChip={`Pressed  F8  →  Theme: ${current.label}`}
        panes={[
          {
            label: `Active Theme: ${current.desc}`,
            active: true,
            pageProps: {
              pageNumber: 2,
              focusSection: 'intro',
              smartDarkPreserveFigures: true,
            },
          },
        ]}
      />

      <div style={{ display: 'flex', gap: 14, fontFamily: FONT_SANS, alignItems: 'center' }}>
        {themes.map((t, i) => (
          <div
            key={t.key}
            style={{
              padding: '9px 18px',
              borderRadius: 11,
              backgroundColor: i === idx ? 'rgba(192, 132, 252, 0.28)' : 'rgba(15, 23, 42, 0.9)',
              border: i === idx ? '2px solid #c084fc' : '1.5px solid rgba(148,163,184,0.28)',
              color: i === idx ? '#f3e8ff' : '#94a3b8',
              fontFamily: FONT_MONO,
              fontSize: 18,
              fontWeight: 800,
            }}
          >
            {t.label}
          </div>
        ))}
        <div
          style={{
            padding: '9px 18px',
            borderRadius: 11,
            backgroundColor: '#020617',
            border: '1.5px solid #334155',
            fontFamily: FONT_MONO,
            fontSize: 17.5,
            fontWeight: 700,
            color: '#93c5fd',
          }}
        >
          config.json: &quot;custom_bg_color&quot;: &quot;#1e1e2e&quot;, &quot;custom_fg_color&quot;: &quot;#cdd6f4&quot;
        </div>
      </div>
    </div>
  );
};

const Scene2SmartDarkPreserve: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const toggleAt = Math.round(durationInFrames * 0.42);
  const preserveOn = frame >= toggleAt;

  return (
    <PinleafWindow
      theme="custom"
      keyPressChip={
        preserveOn
          ? 'Pressed  ⇧F8  →  Smart Dark: Preserve Figures ON'
          : 'Smart Dark OFF (Inverted Plot Bitmap)'
      }
      panes={[
        {
          label: 'Before: Raw Dark Inversion',
          frozen: true,
          pageProps: {
            pageNumber: 4,
            focusSection: 'figure',
            smartDarkPreserveFigures: false,
          },
        },
        {
          label: preserveOn
            ? 'After (⇧F8): True-Color Figure Rects'
            : 'Press ⇧F8 to Preserve Figures…',
          active: true,
          pageProps: {
            pageNumber: 4,
            focusSection: 'figure',
            smartDarkPreserveFigures: preserveOn,
          },
        },
      ]}
      statusBanner={{
        text: 'hayro ImageRectDevice preserves raster plots automatically',
        tone: preserveOn ? 'ok' : 'info',
      }}
    />
  );
};

const Scene3ComfortRulerAutoScroll: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const fitAt = Math.round(durationInFrames * 0.22);
  const rulerAt = Math.round(durationInFrames * 0.44);
  const autoScrollAt = Math.round(durationInFrames * 0.66);

  const fitContent = frame >= fitAt;
  const showRuler = frame >= rulerAt;
  const scrolling = frame >= autoScrollAt;

  const rulerStep = frame < rulerAt + 14 ? 214 : frame < autoScrollAt ? 244 : 296;
  const scrollOffset = scrolling
    ? Math.round(
        interpolate(frame, [autoScrollAt, durationInFrames], [0, 88], {
          extrapolateLeft: 'clamp',
          extrapolateRight: 'clamp',
        }),
      )
    : 0;

  return (
    <PinleafWindow
      keyPressChip={
        scrolling
          ? 'F6 Auto-Scroll Active (⌥+ / ⌥- 1.4×)'
          : showRuler
          ? 'R Reading Ruler (Step j / k)'
          : fitContent
          ? 'F10 / W  →  Fit Content Width'
          : 'Default Fit Page Width (⌘0)'
      }
      zoomLabel={fitContent ? 'Content 142%' : 'Fit 118%'}
      panes={[
        {
          label: fitContent ? 'Fit Content Width • Margins Cropped' : 'Standard Fit Page',
          active: true,
          autoScrollSpeed: scrolling ? 90 : undefined,
          pageProps: {
            pageNumber: 2,
            focusSection: 'intro',
            fitContent,
            readingRulerTopPx: showRuler ? rulerStep : null,
            scrollOffsetPx: scrollOffset,
          },
        },
      ]}
      statusBanner={
        scrolling
          ? { text: '60 FPS Auto-Scroll active • F6 pause • ⌥+/⌥- velocity', tone: 'ok' }
          : undefined
      }
    />
  );
};

const Scene4PresentationMode: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const enterPresentationAt = Math.round(durationInFrames * 0.24);
  const present = frame >= enterPresentationAt;

  return (
    <PinleafWindow
      presentationMode={present}
      keyPressChip="Pressed  F5  →  Fullscreen Presentation Mode"
      panes={[
        {
          label: 'Lecture Slides',
          active: true,
          pageProps: {
            pageNumber: 4,
            focusSection: 'slide',
          },
        },
      ]}
    />
  );
};

export const Video4ThemesComfort: React.FC = () => {
  return (
    <VideoSequenceShell
      video={VIDEO}
      transitionFrames={manifest.transitionFrames}
      renderScene={(idx) => {
        switch (idx) {
          case 0:
            return <Scene1ThemeCycle />;
          case 1:
            return <Scene2SmartDarkPreserve />;
          case 2:
            return <Scene3ComfortRulerAutoScroll />;
          case 3:
          default:
            return <Scene4PresentationMode />;
        }
      }}
    />
  );
};
