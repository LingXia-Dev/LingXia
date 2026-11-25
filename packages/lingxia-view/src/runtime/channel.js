export function createChannel(id, send, close, report, error) {
  let closed = false, failure, iterator = false, bytes = 0;
  const queue = [], reads = [];
  const listeners = { data: new Set(), close: new Set(), error: new Set() };
  const invoke = (fn, ...values) => { try { fn(...values); } catch (cause) { report(cause); } };
  // The close event is `{ error? }`; `error` is set when the channel ended in failure.
  const finish = (cause, failed, discard = false) => {
    if (closed) return;
    closed = true; failure = cause;
    if (cause || discard) { queue.length = 0; bytes = 0; }
    for (const read of reads.splice(0)) {
      if (cause) read.reject(cause); else read.resolve({ done: true, value: undefined });
    }
    if (cause) for (const fn of [...listeners.error]) invoke(fn, cause);
    const event = Object.freeze(failed === undefined ? {} : { error: failed });
    for (const fn of [...listeners.close]) invoke(fn, event);
    for (const bucket of Object.values(listeners)) bucket.clear();
  };
  const handle = Object.freeze({
    id,
    send(payload) {
      if (closed) throw error('STREAM_CLOSED', 'Channel is closed');
      send(payload);
    },
    close(failed) { if (!closed) close(failed); },
    on(kind, fn) {
      if (!Object.prototype.hasOwnProperty.call(listeners, kind) || typeof fn !== 'function')
        throw Error('UI_CHANNEL: invalid listener');
      if (!closed) {
        if (listeners[kind].size >= 256) throw Error('UI_LIMIT: too many channel listeners');
        listeners[kind].add(fn);
        // Logic may push its initial message before the open promise's consumer attaches.
        if (kind === 'data' && !iterator && queue.length) {
          const buffered = queue.splice(0); bytes = 0;
          for (const item of buffered) { if (closed) break; invoke(fn, item.value); }
        }
      }
      return handle;
    },
    [Symbol.asyncIterator]() {
      if (iterator) throw Error('UI_CHANNEL: only one iterator is allowed');
      iterator = true;
      return {
        next() {
          if (queue.length) {
            const item = queue.shift(); bytes -= item.bytes;
            return Promise.resolve({ done: false, value: item.value });
          }
          if (closed) return failure ? Promise.reject(failure) : Promise.resolve({ done: true, value: undefined });
          if (reads.length >= 256) return Promise.reject(error('UI_LIMIT', 'Too many pending channel reads'));
          return new Promise((resolve, reject) => reads.push({ resolve, reject }));
        },
        return() { handle.close(); return Promise.resolve({ done: true, value: undefined }); },
      };
    },
  });
  return {
    handle,
    finish: (failed, discard) => finish(undefined, failed, discard),
    reject: cause => finish(cause, cause),
    push(value) {
      if (closed) return;
      if (reads.length) reads.shift().resolve({ done: false, value });
      else if (iterator || !listeners.data.size) {
        const size = JSON.stringify(value).length * 2;
        if (queue.length >= 256 || bytes + size > 1048576) {
          const limit = error('UI_LIMIT', 'Channel consumer queue exceeded');
          close(limit, limit);
          return;
        }
        queue.push({ value, bytes: size }); bytes += size;
      }
      for (const fn of [...listeners.data]) { if (closed) break; invoke(fn, value); }
    },
  };
}
