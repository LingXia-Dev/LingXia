// A committed local cell owns the connection; abandoned candidates never open it.
export function channelResource(state, report) {
  let definition, channel, ticket = 0, mounted = false;
  let snapshot = state.value;
  const update = patch => { snapshot = { ...snapshot, ...patch }; state.set(snapshot); };
  const close = () => {
    ticket++;
    const old = channel; channel = undefined;
    update({ connecting: false, connected: false });
    old?.close();
  };
  const connect = async () => {
    if (!mounted || snapshot.connecting || snapshot.connected) return;
    const current = ++ticket;
    update({ connecting: true, connected: false, error: null });
    try {
      const opened = await definition.channels.open(definition.topic, definition.params);
      if (!mounted || current !== ticket) { opened.close(); return; }
      channel = opened;
      update({ connecting: false, connected: true });
      opened.on('data', value => {
        if (mounted && current === ticket) definition.onData(value);
      });
      opened.on('error', error => {
        if (mounted && current === ticket) update({ error: String(error.message ?? error) });
      });
      opened.on('close', () => {
        if (mounted && current === ticket) {
          channel = undefined;
          update({ connecting: false, connected: false });
        }
      });
    } catch (error) {
      if (mounted && current === ticket) {
        update({ connecting: false, connected: false, error: String(error.message ?? error) });
      }
    }
  };
  return {
    get state() { return snapshot; },
    connect,
    close,
    send(value) {
      if (!mounted || !channel) throw Error('UI_CHANNEL: resource is disconnected');
      channel.send(value);
    },
    commit(next) {
      const changed = !definition || definition.topic !== next.topic ||
        definition.key !== next.key || definition.channels !== next.channels;
      definition = next;
      if (changed) {
        try { close(); } catch (error) { report(error); }
        mounted = true;
        connect().catch(report);
      }
    },
    dispose() {
      mounted = false;
      try { close(); } catch (error) { report(error); }
    },
  };
}
