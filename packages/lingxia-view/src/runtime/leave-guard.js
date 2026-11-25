import { resourceError } from './errors.js';

// `fail` takes a handler's failure: it fails the page when failures are loud, as a throwing onClick does.
export function createLeaveGuards(callHost, report, fail) {
  const holders = new Set(), backHolders = new Set();
  let epoch = 0, sent = false, sentBack = false, asking = false, tail = Promise.resolve();
  const sync = () => {
    const current = epoch;
    tail = tail.then(async () => {
      if (current !== epoch) return;
      const want = holders.size > 0, wantBack = backHolders.size > 0;
      if (want === sent && wantBack === sentBack) return;
      try {
        // `back`: a back handler is mounted, so Back on the home root comes here too.
        await callHost('navigation.setLeaveGuard', wantBack ? { enabled: want, back: true } : { enabled: want });
        if (current === epoch) { sent = want; sentBack = wantBack; }
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
      holders.clear(); backHolders.clear(); sent = false; sentBack = false; asking = false; tail = Promise.resolve();
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
        fail(error) { update({ error: resourceError(error) }); },
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
    backHandler() {
      let enabled = false, callback = null, registeredEpoch = epoch;
      const holder = {
        cancel() {},
        ask(reason) {
          // Only intercept Back, never Home.
          if (!enabled || reason !== 'back') return Promise.resolve(true);
          const failed = (verb, error) => fail(`UI_HANDLER: onBack ${verb}: ${error instanceof Error ? error.message : String(error)}`);
          try {
            const result = callback?.();
            if (result && typeof result.then === 'function') Promise.resolve(result).catch(error => failed('rejected', error));
          } catch (error) {
            failed('threw', error);
          }
          return Promise.resolve(false);
        },
        fail(error) { report(error); },
      };
      return {
        commit(next) {
          callback = next.onBack;
          if (enabled === next.enabled && registeredEpoch === epoch) return;
          registeredEpoch = epoch;
          enabled = next.enabled;
          if (enabled) { holders.add(holder); backHolders.add(holder); }
          else { holders.delete(holder); backHolders.delete(holder); }
          sync();
        },
        dispose() { enabled = false; callback = null; backHolders.delete(holder); if (holders.delete(holder)) sync(); },
      };
    },
  };
}
