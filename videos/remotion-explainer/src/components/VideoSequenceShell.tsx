import React from 'react';
import { staticFile } from 'remotion';
import { Audio } from '@remotion/media';
import { TransitionSeries, linearTiming } from '@remotion/transitions';
import { fade } from '@remotion/transitions/fade';
import type { SceneMeta, VideoMeta } from '../types';
import { StageFrame } from './StageFrame';

interface VideoSequenceShellProps {
  video: VideoMeta;
  transitionFrames: number;
  renderScene: (sceneIndex: number, scene: SceneMeta) => React.ReactNode;
  hideCaptionForScene?: (sceneIndex: number) => boolean;
}

export const VideoSequenceShell: React.FC<VideoSequenceShellProps> = ({
  video,
  transitionFrames,
  renderScene,
  hideCaptionForScene,
}) => {
  return (
    <TransitionSeries>
      {video.scenes.map((scene, idx) => (
        <React.Fragment key={scene.id}>
          <TransitionSeries.Sequence durationInFrames={scene.durationInFrames}>
            <Audio src={staticFile(scene.audioFile)} />
            <StageFrame
              video={video}
              sceneIndex={idx}
              scene={scene}
              hideCaption={hideCaptionForScene ? hideCaptionForScene(idx) : false}
            >
              {renderScene(idx, scene)}
            </StageFrame>
          </TransitionSeries.Sequence>
          {idx < video.scenes.length - 1 && (
            <TransitionSeries.Transition
              presentation={fade()}
              timing={linearTiming({ durationInFrames: transitionFrames })}
            />
          )}
        </React.Fragment>
      ))}
    </TransitionSeries>
  );
};
