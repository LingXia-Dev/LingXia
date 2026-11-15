import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Avatar',
  category: 'content',
  status: 'preview',
  since: '2026-11-12',
  summary: 'A person or team as a circle: their picture, or their initials',
  description:
    'Avatar shows who: a picture when there is one, initials when there is none or it fails to load. ' +
    'AvatarGroup overlaps several in a row and folds the rest into a count.',
  keywords: ['avatar', 'profile picture', 'photo', 'initials', 'user', 'person', 'member', 'facepile'],
  propsType: 'AvatarProps',
  recipe: { map: 'avatarRecipe', parts: 'avatarParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The circle, on the accent\'s soft step.' },
    { part: 'initials', required: true, description: 'Taken from `name`, or `initials`; under the picture, so a slow load never shows an empty circle.' },
    { part: 'image', required: false, description: 'The picture from `src`, named by `name`; dropped if it fails.' },
    { part: 'group', required: false, description: 'AvatarGroup: a named group, each one cut out of the next by a ring in the surface colour.' },
    { part: 'overflow', required: false, description: 'The "+n" for avatars past `max`, named by `overflowLabel`.' },
  ],
  accessibility: {
    pattern: null,
    name: 'name',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'initials', pair: ['on-accent-soft', 'accent-soft'], minimum: '4.5:1' },
      { what: 'online dot', pair: ['success', 'surface'], minimum: '3:1' },
      { what: 'away dot', pair: ['warning', 'surface'], minimum: '3:1' },
      { what: 'busy dot', pair: ['danger', 'surface'], minimum: '3:1' },
      { what: 'offline dot\'s ring', pair: ['text-muted', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'Read as a picture of the person, by name, whether it shows the photo or the initials. Set `isDecorative` when the name is already written beside it.',
      'The group is read by its label, then each name; the count by the app\'s own words.',
      'A status dot is drawn only: `statusLabel` is read after the name, "Ana Lima, Online". The four statuses differ by colour, and offline also by being hollow, so always pass the label.',
      'A `badge` is read by its own label; say what the number counts in a label beside the avatar or in the row it sits in.',
    ],
  },
  guidance: {
    do: [
      'Mark an avatar decorative when the name sits next to it, so the name is not read twice.',
      'Cap a group with `max` and say what the count means in `overflowLabel`.',
    ],
    dont: [
      'Use an avatar as the only way to tell people apart in a list; show names too.',
      'Make an avatar a button on its own; put it inside an Item or Button that says what it opens.',
    ],
  },
  examples: [
    { name: 'People', title: 'A picture, initials, and a failed picture' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Group', title: 'A group with a count' },
    { name: 'WithName', title: 'Beside the name' },
    { name: 'Presence', title: 'A status dot' },
    { name: 'Unread', title: 'A count on the avatar' },
    { name: 'Teams', title: 'A rounded square for a team' },
  ],
  gaps: [],
  related: ['Item', 'Badge'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Who, at a glance',
    order: 4,
    page: {
      lead: 'Use an avatar to show who did something or who is here, next to their name or in a group.',
      hero: 'People',
      variants: [
        { name: 'Group', title: 'Overlapping, with the rest as a count' },
        { name: 'Presence', title: 'A status dot: online, away, busy, and offline as a hollow dot' },
        { name: 'Unread', title: 'A count on the top end edge' },
        { name: 'Teams', title: 'A rounded square for a team or a workspace, a circle for a person' },
      ],
      states: [
        { name: 'StatePicture', title: 'A picture' },
        { name: 'StateInitials', title: 'No picture' },
        { name: 'StateFailed', title: 'A picture that failed' },
        { name: 'StateOverflow', title: 'Past the group\'s max' },
      ],
      sizes: [{ name: 'Sizes', title: 'Extra small, small, medium and large' }],
      inContext: [{ name: 'WithName', title: 'Beside the name it stands for' }],
    },
  },
};
