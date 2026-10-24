// The render in progress and the committed page, shared by hooks, resolution and encoding.
export const render = { active: false, candidate: null, scope: null, view: null, lookupLocals: null, collectors: [] };
export const page = { committed: null, locals: new Map(), callbacks: new Map(), updates: new Map(), resources: new Map() };

export function outsideRender(what) {
  if (render.active) throw Error(`UI_RENDER: ${what} during render`);
}
