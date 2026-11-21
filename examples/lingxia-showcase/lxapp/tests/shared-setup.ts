import { rawAutomation, skipTestFiles } from '@lingxia/test/runner';
import manifest from './shared-specs.json';

// lxdev.json runs this before any test file: the shared specs read this lxapp's id and renderer
// at import.
const appId = 'lingxia-showcase';
(globalThis as { __SHOWCASE_APP_ID__?: string }).__SHOWCASE_APP_ID__ = appId;
(globalThis as { __SHOWCASE_RENDERER__?: string }).__SHOWCASE_RENDERER__ = 'gpu';
(globalThis as { __SHOWCASE_KEYED_IDS__?: string[] }).__SHOWCASE_KEYED_IDS__ = Object.keys(manifest.keyed);

// Matched under the shared specs' own directory wherever it resolved: a linked or installed
// @lingxia/test lives outside this checkout.
skipTestFiles(manifest.skip, { root: ['lingxia-test/showcase', '@lingxia/test/showcase'] });

// Before the first spec the showcase is open and its current page ready, so its Logic runs. A
// loaded CI runner can take well past 30 s to create the first page.
const deadline = Date.now() + 90_000;
let lastError: unknown = null;
let ready = false;
while (Date.now() < deadline) {
  try {
    const app = rawAutomation().lxapp(appId);
    ready = (await app.info()).appId === appId && (await app.nav.current()).ready;
    if (ready) break;
  } catch (error) {
    lastError = error;
  }
  await new Promise<void>((resolve) => setTimeout(() => resolve(), 100));
}
if (!ready) throw new Error(`Showcase lxapp and Logic runtime did not become ready: ${String(lastError)}`);
