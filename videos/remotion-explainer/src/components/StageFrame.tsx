import React from 'react';
import {
  AbsoluteFill,
  Img,
  interpolate,
  spring,
  staticFile,
  useCurrentFrame,
  useVideoConfig,
} from 'remotion';
import type { SceneMeta, VideoMeta } from '../types';

export const FONT_SANS =
  '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Inter", "Segoe UI", sans-serif';
export const FONT_MONO =
  '"SF Mono", "JetBrains Mono", "Menlo", monospace';

interface StageFrameProps {
  video: VideoMeta;
  sceneIndex: number;
  scene: SceneMeta;
  hideCaption?: boolean;
  children: React.ReactNode;
}

export const StageFrame: React.FC<StageFrameProps> = ({
  video,
  sceneIndex,
  scene,
  hideCaption = false,
  children,
}) => {
  const frame = useCurrentFrame();
  const { fps, durationInFrames } = useVideoConfig();

  const headerEnter = spring({
    frame,
    fps,
    config: { damping: 200 },
    durationInFrames: 18,
  });

  const badgePop = spring({
    frame: Math.max(0, frame - 6),
    fps,
    config: { damping: 16, stiffness: 180 },
  });

  const captionEnter = spring({
    frame: Math.max(0, frame - 4),
    fps,
    config: { damping: 200 },
    durationInFrames: 16,
  });

  const captionExit = interpolate(
    frame,
    [Math.max(0, durationInFrames - 10), durationInFrames],
    [1, 0],
    { extrapolateLeft: 'clamp', extrapolateRight: 'clamp' },
  );

  const orbDrift = interpolate(frame, [0, durationInFrames], [-35, 35], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  const sceneProgress = interpolate(frame, [0, durationInFrames], [0, 1], {
    extrapolateLeft: 'clamp',
    extrapolateRight: 'clamp',
  });

  return (
    <AbsoluteFill
      style={{
        backgroundColor: '#090f1a',
        fontFamily: FONT_SANS,
        color: '#f8fafc',
        overflow: 'hidden',
      }}
    >
      {/* Ambient studio background + subtle grid */}
      <div
        style={{
          position: 'absolute',
          inset: 0,
          background: `radial-gradient(1200px 700px at ${52 + orbDrift * 0.25}% 18%, ${video.accent}26 0%, transparent 65%), radial-gradient(900px 600px at ${18 - orbDrift * 0.15}% 88%, #1d4ed822 0%, transparent 70%), linear-gradient(180deg, #0b1120 0%, #070b14 100%)`,
        }}
      />
      <div
        style={{
          position: 'absolute',
          inset: 0,
          backgroundImage:
            'radial-gradient(rgba(148, 163, 184, 0.14) 1px, transparent 1px)',
          backgroundSize: '36px 36px',
          opacity: 0.45,
        }}
      />

      {/* Top Header Bar */}
      <div
        style={{
          position: 'absolute',
          top: 26,
          left: 48,
          right: 48,
          height: 76,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 24px',
          borderRadius: 18,
          backgroundColor: 'rgba(15, 23, 42, 0.78)',
          border: '1px solid rgba(148, 163, 184, 0.2)',
          boxShadow: '0 14px 40px rgba(0, 0, 0, 0.35)',
          opacity: interpolate(headerEnter, [0, 1], [0.88, 1]),
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <Img
            src={staticFile('pinleaf-icon.png')}
            style={{
              width: 46,
              height: 46,
              borderRadius: 11,
              boxShadow: '0 4px 14px rgba(0,0,0,0.4)',
            }}
          />
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <span
                style={{
                  fontSize: 13,
                  fontWeight: 700,
                  letterSpacing: '0.08em',
                  color: video.accent,
                  textTransform: 'uppercase',
                }}
              >
                Pinleaf Feature Guide • {video.episode}
              </span>
              <span
                style={{
                  fontSize: 12,
                  fontWeight: 600,
                  color: '#94a3b8',
                  padding: '2px 8px',
                  borderRadius: 999,
                  backgroundColor: 'rgba(148, 163, 184, 0.12)',
                }}
              >
                Scene {sceneIndex + 1}/{video.scenes.length}
              </span>
            </div>
            <div style={{ fontSize: 22, fontWeight: 700, letterSpacing: '-0.02em' }}>
              {video.title}
              <span style={{ color: '#64748b', margin: '0 10px' }}>/</span>
              <span style={{ color: '#e2e8f0', fontWeight: 600 }}>{scene.sceneTitle}</span>
            </div>
          </div>
        </div>

        {/* Shortcut badge + step pills */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
            {video.scenes.map((s, idx) => {
              const active = idx === sceneIndex;
              const done = idx < sceneIndex;
              return (
                <div
                  key={s.id}
                  style={{
                    width: active ? 28 : 10,
                    height: 10,
                    borderRadius: 999,
                    backgroundColor: active
                      ? video.accent
                      : done
                      ? `${video.accent}88`
                      : 'rgba(148, 163, 184, 0.25)',
                  }}
                />
              );
            })}
          </div>
          <div
            style={{
              transform: `scale(${interpolate(badgePop, [0, 1], [0.88, 1])})`,
              opacity: badgePop,
              padding: '8px 16px',
              borderRadius: 12,
              backgroundColor: 'rgba(2, 6, 23, 0.85)',
              border: `1.5px solid ${video.accent}88`,
              fontFamily: FONT_MONO,
              fontSize: 17,
              fontWeight: 700,
              color: video.accent,
              boxShadow: `0 0 24px ${video.accent}22`,
              whiteSpace: 'nowrap',
            }}
          >
            {scene.shortcutBadge}
          </div>
        </div>
      </div>

      {/* Main Visual Stage */}
      <div
        style={{
          position: 'absolute',
          top: 120,
          left: 48,
          right: 48,
          bottom: hideCaption ? 34 : 114,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        {children}
      </div>

      {/* Bottom Subtitle / Narration Caption */}
      {!hideCaption && (
        <div
          style={{
            position: 'absolute',
            left: 96,
            right: 96,
            bottom: 24,
            display: 'flex',
            justifyContent: 'center',
            opacity: captionEnter * captionExit,
            transform: `translateY(${interpolate(captionEnter, [0, 1], [10, 0])}px)`,
          }}
        >
          <div
            style={{
              maxWidth: 1460,
              padding: '13px 26px',
              borderRadius: 14,
              backgroundColor: 'rgba(10, 15, 29, 0.9)',
              border: '1px solid rgba(148, 163, 184, 0.22)',
              boxShadow: '0 12px 32px rgba(0,0,0,0.45)',
              fontSize: 23,
              lineHeight: 1.38,
              fontWeight: 500,
              color: '#f1f5f9',
              textAlign: 'center',
            }}
          >
            {scene.caption}
          </div>
        </div>
      )}

      {/* Scene Timeline Accent Strip */}
      <div
        style={{
          position: 'absolute',
          bottom: 0,
          left: 0,
          height: 4,
          width: `${((sceneIndex + sceneProgress) / video.scenes.length) * 100}%`,
          background: `linear-gradient(90deg, ${video.accent}, #38bdf8)`,
        }}
      />
    </AbsoluteFill>
  );
};
