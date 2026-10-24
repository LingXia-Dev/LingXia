export function createLeaveGuards(callHost, report) {
  const holders = new Set();
  let epoch = 0, sent = false, asking = false, tail = Promise.resolve();
  const sync = () => {
    const current = epoch;
    tail = tail.then(async () => {
      if (current !== epoch) return;
      const want = holders.size > 0;
      if (want === sent) return;
      try {
        await callHost('navigation.setLeaveGuard', { enabled: want });
        if (current === epoch) sent = want;
      } catch (error) {
        if (current !== epoch) return;
        for (const holder of holders) holder.fail(error);
        report(error);
      }
    });
  };
  const request = reason => {
    if (!['back', 'home'].includes(reason)) throw Error('UI_LEAVE: invalid reason');
    if (asking) return;
    asking = true;
    const current = epoch, pending = [...holders].reverse();
    (async () => {
      try {
        for (const holder of pending) {
          if (current !== epoch || !holders.has(holder) || !await holder.ask(reason)) return;
        }
        if (current === epoch) await callHost('navigation.leave', { reason });
      } catch (error) {
        if (current === epoch) report(error);
      } finally {
        if (current === epoch) asking = false;
      }
    })();
  };
  return {
    request,
    reset() {
      epoch++;
      for (const holder of holders) holder.cancel();
      holders.clear(); sent = false; asking = false; tail = Promise.resolve();
    },
    resource(state) {
      let snapshot = state.value, answer, enabled = false, mounted = false, registeredEpoch = epoch;
      const update = patch => { snapshot = { ...snapshot, ...patch }; state.set(snapshot); };
      const decide = leave => {
        if (typeof leave !== 'boolean') throw Error('UI_LEAVE: expected boolean answer');
        const resolve = answer; answer = undefined;
        if (!resolve) return;
        update({ pending: null }); resolve(leave);
      };
      const holder = {
        cancel() { decide(false); },
        ask(reason) {
          if (!enabled) return Promise.resolve(false);
          update({ pending: reason, requests: snapshot.requests + 1 });
          return new Promise(resolve => { answer = resolve; });
        },
        fail(error) { update({ error: String(error.message ?? error) }); },
      };
      return {
        get state() { return snapshot; },
        answer: decide,
        request(reason) {
          if (!mounted || registeredEpoch !== epoch) throw Error('UI_LEAVE: stale guard');
          request(reason);
        },
        commit(next) {
          mounted = true;
          if (enabled === next.enabled && registeredEpoch === epoch) return;
          registeredEpoch = epoch;
          enabled = next.enabled;
          if (enabled) { update({ error: null }); holders.add(holder); }
          else { decide(false); holders.delete(holder); }
          sync();
        },
        dispose() { mounted = false; enabled = false; decide(false); if (holders.delete(holder)) sync(); },
      };
    },
  };
}
