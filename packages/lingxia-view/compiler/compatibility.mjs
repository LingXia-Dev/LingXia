import { uiAbi } from '../src/generated/wire.js';

// `minField` is a lower bound from what the page's source names (`fieldFloor`); the runtime's encoder
// refuses any later field the host lacks, so a page is never held to fields it does not write.
export function moduleBanner(compiler, extra = {}) {
  const requiredFeatures = ['candidate-transactions', 'bounded-jobs', ...(extra.requiredFeatures ?? [])];
  const manifest = {
    uiAbi,
    minField: extra.minField ?? 0,
    requiredFeatures,
    compiler,
    ...(extra.chunkBuildId ? { chunkBuildId: extra.chunkBuildId } : {}),
    ...(extra.catalogueChunk ? { catalogueChunk: extra.catalogueChunk } : {}),
    ...(extra.catalogueSource ? { catalogueSource: extra.catalogueSource } : {}),
    ...(extra.catalogueLanguages?.length ? { catalogueLanguages: extra.catalogueLanguages } : {}),
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
