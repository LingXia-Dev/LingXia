// Requirements describe the bundled runtime, including capabilities it can
// emit after state changes. Do not infer a lower baseline from the first frame.
export function moduleBanner(compiler) {
  const manifest = { uiAbi: 1, minField: 108, requiredFeatures: ['candidate-transactions', 'bounded-jobs'], compiler };
  return `// LingXia UI module; ${JSON.stringify(manifest)}
;(function(required) {
  const host = globalThis.__lingxiaUiHost;
  if (!host || host.uiAbi !== required.uiAbi || !Number.isInteger(host.maxField) || host.maxField < required.minField ||
      !Array.isArray(host.features) || required.requiredFeatures.some(name => !host.features.includes(name))) {
    throw Error('UI_COMPAT: this UI bundle requires a compatible native host (ABI ' + required.uiAbi + ', field ' + required.minField + ', ' + required.requiredFeatures.join(', ') + ')');
  }
})(${JSON.stringify(manifest)});`;
}
