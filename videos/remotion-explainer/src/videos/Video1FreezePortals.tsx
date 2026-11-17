import React from 'react';
import { interpolate, spring, useCurrentFrame, useVideoConfig } from 'remotion';
import timingsJson from '../generated/timings.json';
import type { TimingsManifest } from '../types';
import { FONT_MONO, FONT_SANS } from '../components/StageFrame';
import { PinleafWindow } from '../components/PinleafWindow';
import { VideoSequenceShell } from '../components/VideoSequenceShell';

const manifest = timingsJson as TimingsManifest;
const VIDEO = manifest.videos.FreezeAndPortals;

const Scene1FreezeSplit: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();

  const freezeAt = Math.round(durationInFrames * 0.28);
  const splitAt = Math.round(durationInFrames * 0.56);
  const isFrozen = frame >= freezeAt;
  const isSplit = frame >= splitAt;

  const splitEnter = spring({
    frame: Math.max(0, frame - splitAt),
    fps,
    config: { damping: 20, stiffness: 180 },
  });

  const panes = isSplit
    ? [
        {
          label: 'Pane #1 • Intro (Locked)',
          frozen: true,
          pageProps: {
            pageNumber: 1,
            focusSection: 'intro' as const,
            pulseFigureRef: frame >= splitAt + 12,
          },
        },
        {
          label: 'Pane #2 • Theorem Branch',
          active: true,
          pageProps: {
            pageNumber: 3,
            focusSection: 'theorem' as const,
            scrollOffsetPx: Math.round(interpolate(splitEnter, [0, 1], [26, 0])),
          },
        },
      ]
    : [
        {
          label: isFrozen ? 'Pane #1 • Anchored at Page 1' : 'Pane #1 • Main Reading View',
          frozen: isFrozen,
          active: !isFrozen,
          pageProps: { pageNumber: 1, focusSection: 'intro' as const },
        },
      ];

  return (
    <PinleafWindow
      keyPressChip={
        frame < freezeAt
          ? 'Reading Page 1'
          : frame < splitAt
          ? 'Pressed  F  →  Freeze Pane #1 🔒'
          : 'Pressed  ⌘D  →  Freeze & Split Right'
      }
      trailNodes={
        isSplit
          ? [
              { id: 1, frozen: true },
              { id: 2, active: true },
            ]
          : [{ id: 1, frozen: isFrozen, active: !isFrozen }]
      }
      panes={panes}
      statusBanner={
        isSplit
          ? { text: '#1 🔒 pinned while #2 scrolls freely', tone: 'ok' }
          : undefined
      }
    />
  );
};

const Scene2TrailSmartRoute: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const clickAt = Math.round(durationInFrames * 0.48);
  const clicked = frame >= clickAt;

  const cursorX = interpolate(frame, [10, clickAt - 8], [30, 15.5], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });
  const cursorY = interpolate(frame, [10, clickAt - 8], [72, 26], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 14, alignItems: 'center' }}>
      <PinleafWindow
        heightPx={778}
        keyPressChip={clicked ? 'Clicked Theorem 2 in #1 🔒 → Routed #3' : '3-Generation Trail'}
        trailNodes={[
          { id: 1, frozen: true },
          { id: 2, frozen: true },
          { id: 3, active: true },
        ]}
        panes={[
          {
            label: 'Pane #1 • Intro',
            frozen: true,
            pageProps: {
              pageNumber: 1,
              focusSection: 'intro',
              pulseTheoremRef: frame >= clickAt - 12,
            },
          },
          {
            label: 'Pane #2 • Benchmarks',
            frozen: true,
            pageProps: { pageNumber: 3, focusSection: 'figure' },
          },
          {
            label: clicked ? 'Pane #3 • Theorem 2' : 'Pane #3 • Eq. (4)',
            active: true,
            pageProps: {
              pageNumber: clicked ? 6 : 4,
              focusSection: clicked ? 'theorem' : 'equation',
            },
          },
        ]}
        cursor={{
          xPercent: cursorX,
          yPercent: cursorY,
          clicking: frame >= clickAt - 3 && frame <= clickAt + 9,
        }}
      />

      <div style={{ display: 'flex', gap: 16, fontFamily: FONT_SANS }}>
        {[
          { k: '⇧⌘D', l: 'Split Down' },
          { k: '⇧⌘N', l: 'Freeze & New Window' },
          { k: '⌥⌘W', l: 'Detach Pane' },
          { k: '⌘W', l: 'Close Pane' },
        ].map((chip) => (
          <div
            key={chip.k}
            style={{
              padding: '9px 18px',
              borderRadius: 11,
              backgroundColor: 'rgba(15, 23, 42, 0.9)',
              border: '1.5px solid rgba(52, 211, 153, 0.45)',
              fontSize: 19,
              fontWeight: 700,
              color: '#f1f5f9',
              display: 'flex',
              gap: 10,
            }}
          >
            <span style={{ fontFamily: FONT_MONO, fontWeight: 800, color: '#34d399' }}>{chip.k}</span>
            <span>{chip.l}</span>
          </div>
        ))}
      </div>
    </div>
  );
};

const Scene3ScrollPortals: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const linkAt = Math.round(durationInFrames * 0.28);
  const followerAt = Math.round(durationInFrames * 0.56);
  const portalLinked = frame >= linkAt;
  const followerOn = frame >= followerAt;

  return (
    <PinleafWindow
      keyPressChip={
        frame < linkAt
          ? 'Reading Section 1 + Figure 1'
          : frame < followerAt
          ? 'Pressed  P  →  Linked Portal [P1]'
          : 'Pressed  ⇧P  →  Portal Follower ON'
      }
      trailNodes={[
        { id: 1, frozen: false, active: !followerOn },
        { id: 2, active: followerOn },
      ]}
      panes={[
        {
          label: 'Source Pane • Section 1',
          active: true,
          pageProps: {
            pageNumber: 1,
            focusSection: 'intro',
            portalMarginMarkers: portalLinked
              ? [{ topPx: 145, label: 'P1→Fig.1', active: true }]
              : [],
            pulseFigureRef: portalLinked && !followerOn,
          },
        },
        {
          label: followerOn ? 'Helper Pane • Linked Target' : 'Right Pane • Free Anchor',
          portalFollower: followerOn,
          activePortalTarget: followerOn ? 'Figure 1 (p.3)' : undefined,
          pageProps: {
            pageNumber: 3,
            focusSection: 'figure',
          },
        },
      ]}
      statusBanner={
        followerOn
          ? { text: 'P: Link Portal • ⇧P: Follower • ⌥P: Delete', tone: 'info' }
          : undefined
      }
    />
  );
};

const Scene4AutoPortalSwitch: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  const scrollPx = Math.round(
    interpolate(frame, [15, durationInFrames - 20], [0, 120], {
      extrapolateLeft: 'clamp',
      extrapolateRight: 'clamp',
    }),
  );
  const switchAt = Math.round(durationInFrames * 0.48);
  const switchedToEq4 = frame >= switchAt;

  return (
    <PinleafWindow
      keyPressChip={
        switchedToEq4 ? 'Scrolled to [P2] → Auto-Switched' : 'Scrolling Past [P1]…'
      }
      panes={[
        {
          label: 'Source Pane • Continuous Scroll',
          active: true,
          pageProps: {
            pageNumber: 1,
            focusSection: 'intro',
            scrollOffsetPx: scrollPx,
            portalMarginMarkers: [
              { topPx: Math.max(76, 145 - scrollPx * 0.55), label: 'P1→Fig.1', active: !switchedToEq4 },
              { topPx: Math.max(150, 295 - scrollPx * 0.8), label: 'P2→Eq.(4)', active: switchedToEq4 },
            ],
          },
        },
        {
          label: 'Helper Pane • Auto-Synced Portal',
          portalFollower: true,
          activePortalTarget: switchedToEq4 ? 'Eq. (4) (p.4)' : 'Fig. 1 (p.3)',
          pageProps: {
            pageNumber: switchedToEq4 ? 4 : 3,
            focusSection: switchedToEq4 ? 'equation' : 'figure',
          },
        },
      ]}
      statusBanner={{
        text: switchedToEq4
          ? 'Synced [P2] Equation (4) • Saved in LibraryStore'
          : 'Synced [P1] Figure 1 (p.3)',
        tone: 'ok',
      }}
    />
  );
};

export const Video1FreezePortals: React.FC = () => {
  return (
    <VideoSequenceShell
      video={VIDEO}
      transitionFrames={manifest.transitionFrames}
      renderScene={(idx) => {
        switch (idx) {
          case 0:
            return <Scene1FreezeSplit />;
          case 1:
            return <Scene2TrailSmartRoute />;
          case 2:
            return <Scene3ScrollPortals />;
          case 3:
          default:
            return <Scene4AutoPortalSwitch />;
        }
      }}
    />
  );
};
