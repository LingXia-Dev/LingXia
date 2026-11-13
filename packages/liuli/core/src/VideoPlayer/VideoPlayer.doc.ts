import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'VideoPlayer',
  category: 'content',
  status: 'preview',
  summary: 'A video with its controls, over the picture or under it, and full screen',
  description:
    'VideoPlayer shows a `useVideo` player with VideoControls: play, seek, mute, speed and quality, and full screen as a dialog over the page. ' +
    'The page keeps the player, so Logic and the page drive the same video.',
  keywords: ['video', 'player', 'movie', 'stream', 'hls', 'media', 'playback', 'fullscreen'],
  propsType: 'VideoPlayerProps',
  recipe: { map: 'videoPlayerRecipe', parts: 'videoPlayerParts' },
  anatomy: [
    { part: 'video', required: true, description: 'The picture, backed in black where it leaves room.' },
    { part: 'controls', required: true, description: 'VideoControls over the picture, or in a row under it.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    contrast: [{ what: 'controls', pair: ['on-media', 'media'], minimum: '4.5:1' }],
    notes: ['The video is named by `label`; every control is named by `labels`.'],
  },
  guidance: {
    do: [
      'Give it a poster, so the box is never blank before the first frame.',
      'Pass `sources` when the video comes at several qualities.',
    ],
    dont: [
      'Start a video with sound on its own; use `isAutoPlay` only for muted or silent clips.',
    ],
  },
  examples: [
    { name: 'Standard', title: 'Controls over the picture' },
    { name: 'UnderThePicture', title: 'Controls under the picture' },
  ],
  gaps: [],
  related: ['VideoControls'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Watch a video',
    order: 5,
    page: {
      lead: 'Use VideoPlayer to show a video with standard controls.',
      stage: 'bare',
      hero: 'Standard',
      variants: [{ name: 'UnderThePicture', title: 'Controls under the picture' }],
    },
  },
};
