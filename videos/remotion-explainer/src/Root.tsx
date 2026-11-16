import React from 'react';
import { Composition, Folder } from 'remotion';
import timingsJson from './generated/timings.json';
import type { TimingsManifest } from './types';
import { Video1FreezePortals } from './videos/Video1FreezePortals';
import { Video2NavigationHints } from './videos/Video2NavigationHints';
import { Video3AnnotationsLibrary } from './videos/Video3AnnotationsLibrary';
import { Video4ThemesComfort } from './videos/Video4ThemesComfort';
import { Video5SynctexIpc } from './videos/Video5SynctexIpc';

const manifest = timingsJson as TimingsManifest;

export const RemotionRoot: React.FC = () => {
  const { fps, videos } = manifest;
  return (
    <Folder name="Pinleaf-Feature-Explainers">
      <Composition
        id="FreezeAndPortals"
        component={Video1FreezePortals}
        durationInFrames={videos.FreezeAndPortals.totalFrames}
        fps={fps}
        width={1920}
        height={1080}
      />
      <Composition
        id="NavigationSmartJumpHints"
        component={Video2NavigationHints}
        durationInFrames={videos.NavigationSmartJumpHints.totalFrames}
        fps={fps}
        width={1920}
        height={1080}
      />
      <Composition
        id="AnnotationsCategoriesLibrary"
        component={Video3AnnotationsLibrary}
        durationInFrames={videos.AnnotationsCategoriesLibrary.totalFrames}
        fps={fps}
        width={1920}
        height={1080}
      />
      <Composition
        id="ThemesComfortPresentation"
        component={Video4ThemesComfort}
        durationInFrames={videos.ThemesComfortPresentation.totalFrames}
        fps={fps}
        width={1920}
        height={1080}
      />
      <Composition
        id="SynctexIpcExtensions"
        component={Video5SynctexIpc}
        durationInFrames={videos.SynctexIpcExtensions.totalFrames}
        fps={fps}
        width={1920}
        height={1080}
      />
    </Folder>
  );
};
