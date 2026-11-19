import { skipSpecFiles } from '@lingxia/test/runner';
import manifest from './shared-specs.json';

// lxdev.json runs this before any test file: the shared specs read this lxapp's id at import.
(globalThis as { __SHOWCASE_APP_ID__?: string }).__SHOWCASE_APP_ID__ = 'lingxia-showcase-gpu';
(globalThis as { __SHOWCASE_KEYED_IDS__?: string[] }).__SHOWCASE_KEYED_IDS__ = Object.keys(manifest.keyed);

// Matched under the shared specs' own directory wherever it resolved: a linked or installed
// @lingxia/test lives outside this checkout.
skipSpecFiles(manifest.skip, { root: ['lingxia-test/showcase', '@lingxia/test/showcase'] });
