// Streams share the page endpoint; no timers or independent transport live here.
export function createStream(id, cancel, report, error) {
  let done = false, failure, finalValue, iterator = false, bytes = 0;
  const queue = [], reads = [];
  const listeners = { data: new Set(), end: new Set(), error: new Set() };
  let resolve, reject;
  const result = new Promise((yes, no) => { resolve = yes; reject = no; });
  result.catch(() => {});
  const emit = (kind, value) => {
    for (const fn of [...listeners[kind]]) {
      try { fn(value); } catch (cause) { report(cause); }
    }
  };
  const finish = (ok, value) => {
    if (done) return;
    done = true;
    if (ok) { finalValue = value; resolve(value); }
    else { failure = value; queue.length = 0; bytes = 0; reject(value); }
    for (const read of reads.splice(0)) {
      if (ok) read.resolve({ done: true, value }); else read.reject(value);
    }
    emit(ok ? 'end' : 'error', value);
    for (const bucket of Object.values(listeners)) bucket.clear();
  };
  const handle = Object.freeze({
    id, result,
    on(kind, fn) {
      if (!Object.prototype.hasOwnProperty.call(listeners, kind) || typeof fn !== 'function')
        throw Error('UI_STREAM: invalid listener');
      if (!done) {
        if (listeners[kind].size >= 256) throw Error('UI_LIMIT: too many stream listeners');
        listeners[kind].add(fn);
      }
      return handle;
    },
    cancel() { if (!done) cancel(); },
    [Symbol.asyncIterator]() {
      if (iterator) throw Error('UI_STREAM: only one iterator is allowed');
      iterator = true;
      return {
        next() {
          if (queue.length) {
            const item = queue.shift(); bytes -= item.bytes;
            return Promise.resolve({ done: false, value: item.value });
          }
          if (done) return failure ? Promise.reject(failure) : Promise.resolve({ done: true, value: finalValue });
          if (reads.length >= 256) return Promise.reject(error('UI_LIMIT', 'Too many pending stream reads'));
          return new Promise((resolve, reject) => reads.push({ resolve, reject }));
        },
        return() {
          queue.length = 0; bytes = 0;
          handle.cancel();
          return Promise.resolve({ done: true, value: undefined });
        },
      };
    },
  });
  return {
    handle,
    resolve: value => finish(true, value),
    reject: cause => finish(false, cause),
    push(value) {
      if (done) return;
      if (reads.length) reads.shift().resolve({ done: false, value });
      else if (iterator) {
        const size = JSON.stringify(value).length * 2;
        if (queue.length >= 256 || bytes + size > 1048576) {
          cancel(error('UI_LIMIT', 'Stream consumer queue exceeded'));
          return;
        }
        queue.push({ value, bytes: size }); bytes += size;
      }
      emit('data', value);
    },
  };
}
