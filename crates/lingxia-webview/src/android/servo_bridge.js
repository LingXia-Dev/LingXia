(() => {
  const pending = [];
  let activeBeacon = null;
  let sequence = 0;
  const drain = () => {
    if (activeBeacon || pending.length === 0) return;
    const beacon = new Image();
    activeBeacon = beacon;
    const done = () => {
      if (activeBeacon !== beacon) return;
      activeBeacon = null;
      drain();
    };
    beacon.onload = done;
    beacon.onerror = done;
    beacon.src = pending.shift();
  };
  const send = (kind, params) => {
    const query = new URLSearchParams({ ...params, sequence: sequence++ }).toString();
    // Resource requests can reach native code out of order. A single in-flight
    // beacon preserves action/input and component FIFO, including failed images.
    pending.push(`lx://bridge/${kind}?${query}`);
    drain();
  };
  globalThis.LingXiaProxy = {
    supportsMessagePort: () => false,
    getPort: () => '',
    postMessage: message => send('post', { message: String(message) }),
    resolveEval: (id, token, result) => send('eval', { id, token, result }),
  };
  // __LINGXIA_PROFILE_SCRIPT__
})();
