import { resourceError } from './errors.js';

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
  const fail = error => { stream = undefined; update({ status: 'failed', error: resourceError(error) }); };
  return {
    get state() { return snapshot; },
    cancel,
    start(...params) {
      if (!mounted) return;
      cancel();
      const current = ++ticket;
      update({ status: 'streaming', error: null, result: null });
      try {
        if (!definition.action) throw Error('UI_STREAM: bridge not ready');
        stream = definition.action(...params);
        if (definition.onData) stream.on('data', value => {
          if (!mounted || current !== ticket) return;
          try { definition.onData(value); }
          catch (error) { cancel(); fail(error); }
        });
        stream.result.then(value => {
          if (!mounted || current !== ticket) return;
          stream = undefined;
          update({ status: 'completed', result: value ?? null });
        }, error => {
          if (!mounted || current !== ticket) return;
          if (error.code === 'BRIDGE_CANCELED') { stream = undefined; update({ status: 'canceled', error: resourceError(error) }); }
          else fail(error);
        });
      } catch (error) { fail(error); }
    },
    commit(next) {
      if (definition && definition.action !== next.action) {
        cancel();
        update({ status: 'idle', error: null, result: null });
      }
      definition = next; mounted = true;
    },
    dispose() { mounted = false; cancel(); },
  };
}

// A reload keeps the previous value visible and marks it stale until the new one lands.
export function requestResource(state) {
  let definition, mounted = false, ticket = 0, snapshot = state.value;
  const update = patch => { snapshot = { ...snapshot, ...patch }; state.set(snapshot); };
  const reload = async () => {
    if (!mounted || !definition.action) return;
    const current = ++ticket;
    update({ pending: true, stale: snapshot.value !== null, error: null });
    try {
      const value = await definition.action(...(definition.params === undefined ? [] : [definition.params]));
      if (mounted && current === ticket) update({ pending: false, stale: false, value, error: null });
    } catch (error) {
      if (mounted && current === ticket) update({ pending: false, stale: snapshot.value !== null, error: resourceError(error) });
    }
  };
  return {
    get state() { return snapshot; },
    reload,
    commit(next) {
      const changed = !definition || definition.action !== next.action || definition.key !== next.key;
      definition = next; mounted = true;
      if (changed) {
        ticket++;
        if (next.action) void reload();
        else update({ pending: false, stale: false, value: null, error: null });
      }
    },
    dispose() { mounted = false; ticket++; },
  };
}
