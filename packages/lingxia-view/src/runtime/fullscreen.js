// Full screen is drawn by the page: an open <dialog> holding a `<video player>` of the player. A
// page that draws none still gets the picture: the bare video on black, filling the window, which
// a close request leaves.
import { h } from './nodes.js';
import { render } from './state.js';
import { fullscreenPlayer, useVideo } from './video.js';

/** Notes a named player that a `<video>` inside an open dialog shows, on the page or in a list row. */
export function shownInDialog(name) { (render.candidate.fullscreenDrawn ??= new Set()).add(name); }

/** Whether the page draws the player shown full screen in a dialog of its own. */
export function fullscreenDrawn() {
  const name = fullscreenPlayer();
  return name !== null && render.candidate.fullscreenDrawn?.has(name) === true;
}

/** Resolves the bare view, whose own `<video>` does not count as the page drawing one. */
export function resolveBare(resolve) {
  const kept = render.candidate.fullscreenDrawn;
  render.candidate.fullscreenDrawn = new Set(kept);
  try { return resolve(); } finally { render.candidate.fullscreenDrawn = kept; }
}

function BareFullscreen({ name }) {
  const player = useVideo(name);
  return h('dialog', { open: true, closedby: 'closerequest', onClose: () => player.exitFullscreen(), 'aria-label': 'Full screen video',
    'data-testid': 'lx-fullscreen-video',
    style: { width: '100%', height: '100%', maxWidth: '100%', maxHeight: '100%', marginTop: 0, marginBottom: 0, marginLeft: 0, marginRight: 0,
      paddingTop: 0, paddingBottom: 0, paddingLeft: 0, paddingRight: 0, borderRadius: 0, backgroundColor: '#000000' } },
    h('video', { player, style: { width: '100%', flexGrow: 1, objectFit: 'contain' } }));
}

/** The bare full-window view of the player shown full screen, unless the page has drawn its own. */
export function fullscreenFallback() {
  const name = fullscreenPlayer();
  return name === null || fullscreenDrawn() ? null : h(BareFullscreen, { name, key: `lx-fullscreen:${name}` });
}
