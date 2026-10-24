// Only committed definitions can start work; tickets invalidate late results.
export function streamResource(state) {
  let definition, mounted = false, ticket = 0, stream, snapshot = state.value;
  const update = patch => { snapshot = { ...snapshot, ...patch }; state.set(snapshot); };
  const cancel = () => {
    ticket++;
    const old = stream; stream = undefined;
    if (old) {
      old.cancel();
      update({ status: 'canceled' });
    }
  };
  return {
    get state() { return snapshot; },
    cancel,
    start(params) {
      if (!mounted) return;
      cancel();
      const current = ++ticket;
      update({ status: 'streaming', error: null, result: null });
      try {
        if (!definition.action) throw Error('UI_STREAM: bridge not ready');
        stream = definition.action(params);
        stream.on('data', value => {
          if (!mounted || current !== ticket) return;
          try { definition.onData(value); }
          catch (error) {
            cancel();
            update({ status: 'failed', error: String(error.message ?? error) });
          }
        });
        stream.result.then(value => {
          if (!mounted || current !== ticket) return;
          stream = undefined;
          update({ status: 'completed', result: value ?? null });
        }, error => {
          if (!mounted || current !== ticket) return;
          stream = undefined;
          update({ status: error.code === 'BRIDGE_CANCELED' ? 'canceled' : 'failed', error: String(error.message ?? error) });
        });
      } catch (error) {
        stream = undefined;
        update({ status: 'failed', error: String(error.message ?? error) });
      }
    },
    commit(next) {
      if (definition && (definition.action !== next.action || definition.method !== next.method)) {
        cancel();
        update({ status: 'idle', error: null, result: null });
      }
      definition = next; mounted = true;
    },
    dispose() { mounted = false; cancel(); },
  };
}

export function requestResource(state) {
  let definition, mounted = false, ticket = 0, snapshot = state.value;
  const update = patch => { snapshot = { ...snapshot, ...patch }; state.set(snapshot); };
  const reload = async () => {
    if (!mounted || !definition.action) return;
    const current = ++ticket;
    update({ pending: true, value: null, error: null });
    try {
      const value = await definition.action(definition.params);
      if (mounted && current === ticket) update({ pending: false, value, error: null });
    } catch (error) {
      if (mounted && current === ticket)
        update({ pending: false, value: null, error: String(error.message ?? error) });
    }
  };
  return {
    get state() { return snapshot; },
    reload,
    commit(next) {
      const changed = !definition || definition.action !== next.action ||
        definition.method !== next.method || definition.key !== next.key;
      definition = next; mounted = true;
      if (changed) {
        ticket++;
        if (next.action) void reload();
        else update({ pending: false, value: null, error: null });
      }
    },
    dispose() { mounted = false; ticket++; },
  };
}
