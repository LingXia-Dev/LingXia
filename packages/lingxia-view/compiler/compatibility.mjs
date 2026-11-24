import { uiAbi, minField } from '../src/generated/wire.js';

// Requirements describe the bundled runtime, including fields it can emit after
// state changes. Do not infer a lower baseline from the first frame.
export function moduleBanner(compiler, extra = {}) {
  const requiredFeatures = ['candidate-transactions', 'bounded-jobs', ...(extra.requiredFeatures ?? [])];
  const manifest = {
    uiAbi,
    minField,
    requiredFeatures,
    compiler,
    ...(extra.chunkBuildId ? { chunkBuildId: extra.chunkBuildId } : {}),
    ...(extra.catalogueChunk ? { catalogueChunk: extra.catalogueChunk } : {}),
  };
  return `// LingXia UI module; ${JSON.stringify(manifest)}
;(function(required) {
  const host = globalThis.__lingxiaViewHost;
  if (!host || host.uiAbi !== required.uiAbi || !Number.isInteger(host.maxField) || host.maxField < required.minField ||
      !Array.isArray(host.features) || required.requiredFeatures.some(name => !host.features.includes(name))) {
    throw Error('UI_COMPAT: this UI bundle requires a compatible native host (ABI ' + required.uiAbi + ', field ' + required.minField + ', ' + required.requiredFeatures.join(', ') + ')');
  }
})(${JSON.stringify(manifest)});`;
}
