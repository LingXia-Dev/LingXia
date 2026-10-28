import { skipSpecFiles } from '@lingxia/test/runner';
import manifest from './shared-specs.json';

// Runs before the shared specs load: they read this lxapp's id at import.
(globalThis as { __SHOWCASE_APP_ID__?: string }).__SHOWCASE_APP_ID__ = 'lingxia-showcase-gpu';
(globalThis as { __SHOWCASE_KEYED_IDS__?: string[] }).__SHOWCASE_KEYED_IDS__ = Object.keys(manifest.keyed);

// Source names are relative to this project's root (lxapp/), where the
// manifest's source is relative to tests/.
const root = manifest.source.replace(/^\.\.\//, '');
skipSpecFiles(Object.fromEntries(
  Object.entries(manifest.skip).map(([file, reason]) => [`${root}/${file}`, reason]),
));
