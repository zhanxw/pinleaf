export interface SceneMeta {
  id: string;
  sceneTitle: string;
  shortcutBadge: string;
  caption: string;
  audioFile: string;
  audioSeconds: number;
  durationInFrames: number;
}

export interface VideoMeta {
  id: string;
  filename: string;
  episode: string;
  title: string;
  subtitle: string;
  accent: string;
  totalFrames: number;
  durationSeconds: number;
  scenes: SceneMeta[];
}

export interface TimingsManifest {
  fps: number;
  transitionFrames: number;
  videos: Record<string, VideoMeta>;
}
