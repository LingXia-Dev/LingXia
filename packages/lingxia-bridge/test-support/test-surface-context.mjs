import assert from 'node:assert/strict';

// Two copies of the bridge module live in a page: the injected runtime and
// the page's own bundle. Both must read the one store the host writes.
// The host seeds the context into the document's bridge config.
globalThis.window = {
  __LX_BRIDGE_CFG: {
    surfaceContext: { sizeClass: 'regular', width: 800, height: 600, aside: false },
    surfaceContextRevision: 5,
  },
};
const injected = await import('../dist/es2020/surface-context.js?copy=injected');
const bundled = await import('../dist/es2020/surface-context.js?copy=bundled');

assert.deepEqual(
  bundled.getSurfaceContext(),
  { sizeClass: 'regular', width: 800, height: 600, aside: false },
  'the seed is there from the first frame',
);

let heard = 0;
const unsubscribe = bundled.subscribeSurfaceContext(() => {
  heard += 1;
});
const apply = window.__lingxiaApplySurfaceContext;
assert.equal(typeof apply, 'function', 'the host entry point is installed');

apply({ sizeClass: 'compact', width: 300, height: 600, aside: false }, 4);
assert.equal(bundled.getSurfaceContext().width, 800, 'a push older than the seed is stale');
assert.equal(heard, 0);

apply({ sizeClass: 'regular', width: 900, height: 700, aside: true });
assert.deepEqual(injected.getSurfaceContext(), { sizeClass: 'regular', width: 900, height: 700, aside: true });
assert.equal(bundled.getSurfaceContext(), injected.getSurfaceContext(), 'both copies share one store');
assert.equal(heard, 1);

const before = bundled.getSurfaceContext();
apply({ sizeClass: 'regular', width: 900, height: 700, aside: true });
assert.equal(heard, 1, 'an unchanged context notifies nobody');
assert.equal(bundled.getSurfaceContext(), before, 'and keeps its identity for useSyncExternalStore');

apply({ sizeClass: 'wide', width: 1, height: 1 });
apply(null);
assert.equal(heard, 1, 'a malformed push is ignored');

apply({ sizeClass: 'compact', width: 390, height: 844, aside: false });
assert.equal(heard, 2);
assert.notEqual(bundled.getSurfaceContext(), before, 'a change is a new object');

// The host numbers its pushes: one built earlier but run later is stale.
apply({ sizeClass: 'regular', width: 1200, height: 800, aside: false }, 10);
apply({ sizeClass: 'compact', width: 390, height: 844, aside: false }, 9);
assert.equal(bundled.getSurfaceContext().width, 1200, 'an older revision never overwrites a newer one');
assert.equal(heard, 3);

unsubscribe();
apply({ sizeClass: 'regular', width: 1000, height: 800, aside: false }, 11);
assert.equal(heard, 3, 'unsubscribed listeners stay quiet');


// A host older than this API seeds nothing: a valid default, never null.
{
  globalThis.window = {};
  const older = await import('../dist/es2020/surface-context.js?copy=older-host');
  assert.deepEqual(older.getSurfaceContext(), { sizeClass: 'compact', width: 0, height: 0, aside: false });
}

console.log('surface context store: ok');
