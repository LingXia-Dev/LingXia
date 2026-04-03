// Plugin registry + requirePlugin helper (non-ESM).
(function () {
  const registry = Object.create(null);
  const loaders = Object.create(null);

  function normalizeName(name) {
    if (typeof name !== "string") {
      throw new Error("requirePlugin: plugin name must be a string");
    }
    const trimmed = name.trim();
    if (!trimmed) {
      throw new Error("requirePlugin: plugin name is empty");
    }
    return trimmed;
  }

  globalThis.__LX_PLUGIN_REGISTRY__ = registry;

  globalThis.registerPlugin = function (name, api) {
    const pluginName = normalizeName(name);
    registry[pluginName] = api;
    return api;
  };

  globalThis.requirePlugin = async function (name) {
    const pluginName = normalizeName(name);
    if (registry[pluginName]) {
      return registry[pluginName];
    }
    if (loaders[pluginName]) {
      await loaders[pluginName];
      const cached = registry[pluginName];
      if (!cached) {
        throw new Error(`requirePlugin: plugin did not register API: ${pluginName}`);
      }
      return cached;
    }
    const loader = globalThis.__LX_REQUIRE_PLUGIN__;
    if (typeof loader !== "function") {
      throw new Error("requirePlugin: native loader missing");
    }
    const pending = Promise.resolve()
      .then(() => loader(pluginName))
      .finally(() => {
        delete loaders[pluginName];
      });
    loaders[pluginName] = pending;
    await pending;
    const api = registry[pluginName];
    if (!api) {
      throw new Error(`requirePlugin: plugin did not register API: ${pluginName}`);
    }
    return api;
  };
})();
