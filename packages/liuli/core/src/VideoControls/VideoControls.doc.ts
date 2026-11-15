import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'VideoControls',
  category: 'media',
  status: 'preview',
  since: '2026-11-13',
  summary: 'Play, seek, mute, speed, quality and full screen for a useVideo player',
  description:
    'VideoControls drives a `useVideo` player: over the picture as a quiet bar that hides on touch while it plays, or as a row under it. ' +
    'Full screen is a dialog over the page that shows the same player again with these controls. Prefer `VideoPlayer`, which pairs the picture and the controls.',
  keywords: ['video', 'player', 'controls', 'scrubber', 'seek', 'media', 'playback', 'fullscreen'],
  propsType: 'VideoControlsProps',
  otherPropsTypes: ['VideoControlsLabels'],
  recipe: { map: 'videoControlsRecipe', parts: 'videoControlsParts' },
  anatomy: [
    { part: 'surface', required: true, description: 'The picture under the bar: a tap shows or hides the bar on touch, plays or pauses with a mouse.' },
    { part: 'bar', required: true, description: 'Media glass at the bottom edge: the scrubber line, then play and the time, then settings, mute and full screen. Under the picture, one row: play, the slider, the time, settings, mute and full screen.' },
    { part: 'scrubber', required: false, description: 'A slider whose touch box grows up into the picture; it thickens and shows its thumb while pressed. Absent for live streams.' },
    { part: 'centre', required: false, description: 'A large play before the first play, a ring from a play tap until the player answers and while loading, or the error with Retry.' },
    { part: 'menu', required: false, description: 'Speed and quality, a row of chips each, in the bar\'s place while open.' },
    { part: 'fullscreen', required: false, description: 'A dialog showing the player again, full window, with the same controls and a way out.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [
      { state: 'pressed', from: 'mute and full screen' },
      { state: 'expanded', from: 'the settings menu' },
      { state: 'busy', from: 'play while the player loads' },
    ],
    keyboard: [
      { keys: 'Arrow keys on the scrubber', effect: 'seek by 0.1 s steps' },
      { keys: 'Escape', effect: 'leaves full screen' },
    ],
    contrast: [
      { what: 'icons and time', pair: ['on-media', 'media'], minimum: '4.5:1' },
      { what: 'progress line', pair: ['on-media', 'media'], minimum: '3:1' },
      { what: 'progress against the unplayed track', pair: ['on-media', 'on-media/25'], over: 'media', minimum: '3:1' },
      { what: 'chosen speed or quality', pair: ['media', 'on-media'], minimum: '4.5:1' },
      { what: 'speed or quality under the picture', pair: ['text', 'text/8'], over: 'surface', minimum: '4.5:1' },
      { what: 'chosen speed or quality under the picture', pair: ['on-accent', 'accent'], minimum: '4.5:1' },
    ],
    notes: [
      'The scrubber reads its position as "m:ss / m:ss" and runs left to right in every language.',
      'Over the picture the accent is not used: progress, the chosen chip and LIVE paint in the media roles.',
      'Faded controls stay in the tree, so a screen reader reaches every one of them.',
    ],
  },
  guidance: {
    do: [
      'Put overlay controls after the `<video>` in a `relative` parent that holds only that video.',
      'Pass the `<video>`\'s poster so full screen shows it before the first frame.',
      'Pass every label in the page\'s language.',
    ],
    dont: [
      'Lay anything else over the picture between the video and its controls.',
      'Put a backdrop blur behind the bar; the picture moves.',
    ],
  },
  examples: [
    { name: 'Overlay', title: 'Over the picture' },
    { name: 'Bar', title: 'As a row under the picture' },
  ],
  gaps: [
    {
      missing: 'Hiding the bar when a mouse leaves the picture.',
      needs: 'pointer enter and leave on components',
      today: 'with a mouse the bar stays up.',
    },
    {
      missing: 'Double-tap to skip.',
      needs: 'a double-tap gesture',
      today: 'drag the scrubber.',
    },
  ],
  related: ['VideoPlayer', 'Slider'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Drive a video',
    order: 2,
    page: {
      lead: 'Use VideoControls beside your own <video> when VideoPlayer does not fit the layout.',
      stage: 'bare',
      hero: 'Overlay',
      variants: [{ name: 'Bar', title: 'A row under the picture' }],
    },
  },
};
