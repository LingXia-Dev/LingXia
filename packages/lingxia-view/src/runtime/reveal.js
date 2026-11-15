// `reveal(id)`: a handler asks for the element with that `id` to be scrolled into view. The host
// takes the requests after the render the handler caused and moves the scrolls around the element.
import { outsideRender } from './state.js';

let pending = [];
const MOST = 16;

export function reveal(id) {
  outsideRender('reveal');
  if (typeof id !== 'string' || !id.length || id.length > 256) throw Error('UI_ID: reveal takes an element id of 1..256 characters');
  if (pending.length < MOST) pending.push(id);
}

export function takeReveals() { const out = pending; pending = []; return JSON.stringify(out); }

export function resetReveals() { pending = []; }
