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
          top: 14,
          left: 22,
          right: 22,
          height: 74,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 22px',
          borderRadius: 16,
          backgroundColor: 'rgba(15, 23, 42, 0.84)',
          border: '1px solid rgba(148, 163, 184, 0.24)',
          boxShadow: '0 14px 40px rgba(0, 0, 0, 0.35)',
          opacity: interpolate(headerEnter, [0, 1], [0.88, 1]),
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <Img
            src={staticFile('pinleaf-icon.png')}
            style={{
              width: 48,
              height: 48,
              borderRadius: 12,
              boxShadow: '0 4px 14px rgba(0,0,0,0.4)',
            }}
          />
          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
              <span
                style={{
                  fontSize: 15,
                  fontWeight: 800,
                  letterSpacing: '0.07em',
                  color: video.accent,
                  textTransform: 'uppercase',
                }}
              >
                Pinleaf Guide • {video.episode}
              </span>
              <span
                style={{
                  fontSize: 14,
                  fontWeight: 700,
                  color: '#cbd5e1',
                  padding: '2px 10px',
                  borderRadius: 999,
                  backgroundColor: 'rgba(148, 163, 184, 0.16)',
                }}
              >
                Scene {sceneIndex + 1}/{video.scenes.length}
              </span>
            </div>
            <div style={{ fontSize: 27, fontWeight: 800, letterSpacing: '-0.02em', lineHeight: 1.15 }}>
              {video.title}
              <span style={{ color: '#64748b', margin: '0 10px' }}>/</span>
              <span style={{ color: '#e2e8f0', fontWeight: 700 }}>{scene.sceneTitle}</span>
            </div>
          </div>
        </div>

        {/* Shortcut badge + step pills */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <div style={{ display: 'flex', gap: 7, alignItems: 'center' }}>
            {video.scenes.map((s, idx) => {
              const active = idx === sceneIndex;
              const done = idx < sceneIndex;
              return (
                <div
                  key={s.id}
                  style={{
                    width: active ? 30 : 11,
                    height: 11,
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
              transform: `scale(${interpolate(badgePop, [0, 1], [0.9, 1])})`,
              opacity: badgePop,
              padding: '8px 18px',
              borderRadius: 12,
              backgroundColor: 'rgba(2, 6, 23, 0.9)',
              border: `2px solid ${video.accent}aa`,
              fontFamily: FONT_MONO,
              fontSize: 23,
              fontWeight: 800,
              color: video.accent,
              boxShadow: `0 0 24px ${video.accent}28`,
              whiteSpace: 'nowrap',
            }}
          >
            {scene.shortcutBadge}
          </div>
        </div>
      </div>

      {/* Main Visual Stage (1876 x 876) */}
      <div
        style={{
          position: 'absolute',
          top: 98,
          left: 22,
          right: 22,
          bottom: hideCaption ? 22 : 106,
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
            left: 32,
            right: 32,
            bottom: 12,
            display: 'flex',
            justifyContent: 'center',
            opacity: captionEnter * captionExit,
            transform: `translateY(${interpolate(captionEnter, [0, 1], [10, 0])}px)`,
          }}
        >
          <div
            style={{
              maxWidth: 1820,
              padding: '10px 26px',
              borderRadius: 14,
              backgroundColor: 'rgba(10, 15, 29, 0.94)',
              border: '1px solid rgba(148, 163, 184, 0.28)',
              boxShadow: '0 12px 32px rgba(0,0,0,0.48)',
              fontSize: 27,
              lineHeight: 1.28,
              fontWeight: 600,
              color: '#f8fafc',
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
          height: 5,
          width: `${((sceneIndex + sceneProgress) / video.scenes.length) * 100}%`,
          background: `linear-gradient(90deg, ${video.accent}, #38bdf8)`,
        }}
      />
    </AbsoluteFill>
  );
};
