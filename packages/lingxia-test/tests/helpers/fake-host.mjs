const TINY_PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

function testIdFromCss(css) {
  const match = String(css).match(/\[data-testid=["']?([^"'\]]+)/);
  return match ? match[1].replace(/\\/g, "") : undefined;
}

/**
 * Mirrors the host's dialog watch: `driver` is `lxapp().dialogs`, and
 * `logic` is what the app's Logic does when it calls `lx.showToast`,
 * `lx.showModal` or `lx.showActionSheet`. Watched, a dialog is recorded and
 * drawn until the spec queued an answer of its kind, then answered or
 * refused. A drawn modal or sheet returns `{ drawn, close(choice) }`: the
 * user's tap. Unwatched, `"presented"`.
 */
function createDialogs() {
  let watch;
  const end = () => {
    for (const wake of watch?.waiters ?? []) wake(null);
    watch = undefined;
  };
  const watched = () => {
    if (!watch) throw new Error("dialogs of demo-app are not watched");
    return watch;
  };
  const fail = (message) => {
    if (watch.failure !== undefined) return;
    watch.failure = message;
    for (const wake of watch.waiters.splice(0)) wake(message);
  };
  const driver = {
    watch() {
      end();
      watch = { toasts: [], modals: [], sheets: [], modalAnswers: [], sheetAnswers: [], answeringModals: false, answeringSheets: false, waiters: [], failure: undefined };
    },
    unwatch() {
      const left = { modalAnswers: watch?.modalAnswers.length ?? 0, actionSheetAnswers: watch?.sheetAnswers.length ?? 0 };
      end();
      return left;
    },
    async unanswered() {
      const current = watched();
      if (current.failure !== undefined) return current.failure;
      return new Promise((resolve) => current.waiters.push(resolve));
    },
    toasts: () => structuredClone(watched().toasts),
    modals: () => structuredClone(watched().modals),
    actionSheets: () => structuredClone(watched().sheets),
    answerNextModal(answer) {
      if (typeof answer?.confirm !== "boolean" || Object.keys(answer).length !== 1) throw new Error("answerNextModal takes { confirm: true | false }");
      watched().modalAnswers.push(answer.confirm);
      watch.answeringModals = true;
    },
    answerNextActionSheet(answer) {
      const valid = answer && Object.keys(answer).length === 1 &&
        (answer.cancel === true || (Number.isInteger(answer.index) && answer.index >= 0));
      if (!valid) throw new Error("answerNextActionSheet takes { index } or { cancel: true }");
      watched().sheetAnswers.push(answer);
      watch.answeringSheets = true;
    },
  };
  const logic = {
    showToast({ title, icon = "none", durationMs = 1500 }) {
      watch?.toasts.push({ title, icon, duration: durationMs, at: Date.now() });
      return "presented";
    },
    showModal({ title = "", content = "", confirmText, cancelText, showCancel = true }) {
      if (!watch) return "presented";
      const confirm = watch.modalAnswers.shift();
      const record = { title, content, answer: confirm === undefined ? null : { confirm } };
      if (confirmText !== undefined) record.confirmText = confirmText;
      if (cancelText !== undefined && showCancel) record.cancelText = cancelText;
      watch.modals.push(record);
      if (!watch.answeringModals) {
        record.drawn = true;
        return { drawn: true, close: (choice) => { record.answer = { confirm: choice }; } };
      }
      if (confirm !== undefined) return { confirm };
      const message = `a modal appeared with no answer queued: title ${JSON.stringify(title)}, content ${JSON.stringify(content)}`;
      fail(message);
      throw new Error(message);
    },
    showActionSheet(items) {
      if (!watch) return "presented";
      const answer = watch.sheetAnswers.shift();
      const record = { items, answer: answer ?? null };
      watch.sheets.push(record);
      if (!watch.answeringSheets) {
        record.drawn = true;
        return { drawn: true, close: (choice) => { record.answer = choice; } };
      }
      if (answer?.cancel) return { cancel: true };
      if (answer && answer.index < items.length) return { index: answer.index };
      const message = answer ? `the queued action sheet answer { index: ${answer.index} } is outside its ${items.length} items`
        : `an action sheet appeared with no answer queued: items ${JSON.stringify(items)}`;
      fail(message);
      throw new Error(message);
    },
  };
  return { driver, logic, get watching() { return watch !== undefined; } };
}

export function createWorld(options = {}) {
  const elements = options.elements ? [...options.elements] : [];
  const evalResults = new Map();
  const evalCalls = new Map();
  let instances = 0;
  // Every landing is a new page instance, as navigation is on a real host.
  const landedPage = (name) =>
    ({ name, path: `pages/${name}/index`, instanceId: `p${++instances}`, current: true, inStack: true, ready: true, webviewAttached: true });
  let currentPage = landedPage("home");
  const stack = [currentPage];
  /** Every nav action as `[method, options]`, to check what the fixture sent. */
  const navCalls = [];
  let relaunchError;
  let blocked = false;
  /** Globals a script sees in Logic / the WebView; unset, scripts echo back. */
  let logicGlobals;
  let pageGlobals;
  const evaluated = [];
  // The `page` each page eval named (`undefined`: the current page).
  const evaluatedPages = [];
  // The `timeoutMs` each eval (Logic or page) was sent with.
  const evalTimeouts = [];
  const keyFor = (map, script) => map.has(script) ? script : [...map.keys()].find((key) => script.includes(key));

  // Mirror the targets: a script is evaluated as JS against the given
  // globals, and a thrown error reaches the test context as a plain Error
  // whose message carries the remote name, as the host's eval error does.
  async function evaluate(globals, script) {
    evaluated.push(script);
    const names = Object.keys(globals);
    try {
      const run = new Function(...names, `return eval(${JSON.stringify(script)});`);
      return await run(...names.map((name) => globals[name]));
    } catch (error) {
      const remote = new Error(`${error?.name ?? "Error"}: ${error?.message ?? error}`);
      remote.code = "E_EVAL";
      throw remote;
    }
  }

  function matches(element, css) {
    const testId = testIdFromCss(css);
    if (testId) return element.testId === testId;
    if (css.startsWith("#")) return element.id === css.slice(1);
    if (element.css && element.css === css) return true;
    return element.tag === css;
  }

  function queryAll(css) {
    return elements.filter((element) => element.attached !== false && matches(element, css));
  }

  const page = {
    async query({ css, all, index = 0 }) {
      const found = queryAll(css);
      if (all) {
        return {
          count: found.length,
          items: found.map((element, i) => serialize(element, i, found.length)),
        };
      }
      const element = found[index];
      if (!element) {
        return { exists: false, index, count: found.length, visible: false, enabled: false, editable: false };
      }
      return serialize(element, index, found.length);
    },
    async click({ css, index, force }) {
      if (blocked) throw new Error("fixture should not reach the app after abort");
      const found = queryAll(css);
      const target = typeof index === "number" ? found[index] : found.find((element) => element.visible !== false);
      if (!target || (target.visible === false && !force)) throw new Error(`click missed ${css}`);
      target.clicked = (target.clicked ?? 0) + 1;
      target.forced = force === true;
      if (typeof target.onClick === "function") target.onClick(target);
    },
    async fill({ css, text, index, force }) {
      if (blocked) throw new Error("fixture should not reach the app after abort");
      const found = queryAll(css);
      const target = typeof index === "number" ? found[index] : found.find((element) => element.visible !== false);
      if (!target || (target.visible === false && !force)) throw new Error(`fill missed ${css}`);
      target.value = text;
      target.forced = force === true;
    },
    async type({ css, text }) {
      return page.fill({ css, text });
    },
    async screenshot() {
      return { format: "png", base64: TINY_PNG, width: 1, height: 1 };
    },
    async eval({ script, page: target, timeoutMs }) {
      evaluatedPages.push(target);
      evalTimeouts.push(timeoutMs);
      if (pageGlobals) return evaluate(pageGlobals, script);
      // The locator's read-only attribute probe.
      const probe = script.match(/querySelectorAll\((".*?")\)\[(\d+)\][\s\S]*for \(const name of (\[.*?\])\)/);
      if (probe) {
        const element = queryAll(JSON.parse(probe[1]))[Number(probe[2])];
        const out = {};
        for (const name of JSON.parse(probe[3])) out[name] = element?.attributes?.[name] ?? null;
        return out;
      }
      // The locator's actionability probe: an element outside the viewport
      // fails the hit test, as a real page would.
      const actionability = script.match(/querySelectorAll\((".*?")\)\[(\d+)\][\s\S]*elementFromPoint/);
      if (actionability) {
        const element = queryAll(JSON.parse(actionability[1]))[Number(actionability[2])];
        if (element?.inViewport === false) return "element is obscured";
      }
      return true;
    },
  };

  const nav = {
    async relaunch(options) {
      navCalls.push(["relaunch", options]);
      if (blocked) throw new Error("fixture should not reach the app after abort");
      if (relaunchError) throw relaunchError;
      currentPage = landedPage(options.page);
      stack.splice(0, stack.length, currentPage);
      return currentPage;
    },
    async current() {
      return currentPage;
    },
    async stack() {
      return stack.map(entry => ({ ...entry }));
    },
    async info(options) {
      const name = options?.page;
      if (name === undefined) return currentPage;
      return stack.find((entry) => entry.name === name || entry.instanceId === name)
        ?? { name, path: `pages/${name}/index`, instanceId: null, current: false, inStack: false, ready: false, webviewAttached: false };
    },
    async to(options) {
      navCalls.push(["to", options]);
      currentPage = landedPage(options.page);
      stack.push(currentPage);
      return currentPage;
    },
    async back(options) {
      navCalls.push(["back", options]);
      if (stack.length > 1) stack.pop();
      currentPage = stack[stack.length - 1];
      return currentPage;
    },
  };

  const dialogs = createDialogs();
  const app = {
    page,
    nav,
    dialogs: dialogs.driver,
    async info() {
      return {
        appid: options.appId ?? "demo-app",
        app_name: "Demo",
        version: "0.0.0",
        release_type: "developer",
        session_id: 1,
        status: "running",
        is_home: true,
        current_page: currentPage.path,
        initial_route: "pages/home/index",
        pages_count: 1,
        page_entries: [{ name: "home", path: "pages/home/index" }],
        page_stack: stack.map((item) => item.path),
        tab_bar: null,
        lxapp_dir: "",
        data_dir: "",
        cache_dir: "",
      };
    },
    async pages() {
      return [{ name: "home", path: "pages/home/index" }];
    },
    async surfaceLayout() {
      return { sizeClass: "compact", mains: ["main"] };
    },
    async eval({ script, captureCalls, timeoutMs }) {
      evalTimeouts.push(timeoutMs);
      if (blocked) throw new Error("fixture should not reach the app after abort");
      let value = script;
      // A seeded key names the script, or text inside the function a spec
      // passed to `t.app.logic.eval`.
      const seeded = keyFor(evalResults, script);
      if (logicGlobals && seeded === undefined) value = await evaluate(logicGlobals, script);
      if (seeded !== undefined) {
        value = evalResults.get(seeded);
        if (value instanceof Error) throw value;
      }
      // Mirror the runtime: with `captureCalls` the result is wrapped and
      // carries what the script reached. Tests seed that through `setCalls`.
      if (captureCalls) {
        // Mirror the runtime: `value` is absent when the script returns undefined.
        const calls = keyFor(evalCalls, script);
        const envelope = { __lxEval: 1, calls: calls === undefined ? [] : evalCalls.get(calls) };
        if (value !== undefined) envelope.value = value;
        return envelope;
      }
      return value;
    },
  };

  function serialize(element, index, count) {
    return {
      exists: true,
      index,
      count,
      tag: element.tag ?? "div",
      type: element.type ?? null,
      id: element.id ?? null,
      name: element.name ?? null,
      role: element.role ?? null,
      aria_label: null,
      placeholder: null,
      visible: element.visible !== false,
      inViewport: element.visible !== false && element.inViewport !== false,
      enabled: element.enabled !== false,
      editable: element.editable !== false,
      text: element.text ?? "",
      text_truncated: false,
      value: element.value ?? null,
      value_truncated: false,
      rect: { left: 0, top: 0, width: 10, height: 10, right: 10, bottom: 10, center_x: 5, center_y: 5, viewport_width: 100, viewport_height: 100 },
    };
  }

  return {
    TINY_PNG,
    elements,
    app,
    /** The app's Logic opening dialogs (`logic`), and whether the runner watches them. */
    dialogs,
    navCalls,
    /** Make the next relaunches reject with `error` (undefined restores). */
    failRelaunch(error) {
      relaunchError = error;
    },
    add(element) {
      elements.push({ attached: true, visible: true, ...element });
      return elements[elements.length - 1];
    },
    setEval(script, value) {
      evalResults.set(script, value);
    },
    /** What the runtime should report the script reached. */
    setCalls(script, calls) {
      evalCalls.set(script, calls);
    },
    block() {
      blocked = true;
    },
    /** Evaluate Logic scripts against these globals (`lx`, `getCurrentPages`, …). */
    useLogic(globals) {
      logicGlobals = globals;
    },
    /** Evaluate page scripts against these globals (`document`, `window`). */
    usePage(globals) {
      pageGlobals = globals;
    },
    /** Every script actually evaluated, in order. */
    evaluated,
    evaluatedPages,
    evalTimeouts,
    unblock() {
      blocked = false;
    },
  };
}

/** Methods that install something the host scopes to the open attempt. */
const INSTALLS = new Set(["route", "use", "install"]);

/**
 * Mirrors the host's spec attempts and revoke: with `options.attempts`, what
 * a driver installs while an attempt is open belongs to it and is removed
 * when it ends; installs between attempts are refused; after `revoke` every
 * driver call of the context rejects with `E_AUTOMATION_PRIVILEGE`.
 */
function attemptHost(options) {
  const state = {
    next: 0,
    open: undefined,
    opened: false,
    revoked: undefined,
    /** Every install, `{ attempt, what, handle }`. */
    installs: [],
    /** `endAttempt` rejects with this once (a host that cannot remove). */
    failSweep: undefined,
    /** Driver calls the host refused, as `path`. */
    refused: [],
  };
  const refuse = (path) => {
    state.refused.push(path);
    return Object.assign(new Error(`E_AUTOMATION_PRIVILEGE: automation access of this run was revoked: ${state.revoked}`), {
      code: "E_AUTOMATION_PRIVILEGE",
    });
  };
  const sweep = async (token) => {
    const mine = state.installs.filter((entry) => entry.attempt === token);
    state.installs = state.installs.filter((entry) => entry.attempt !== token);
    const counts = { routes: 0, scenarios: 0, clocks: 0, droppedTimers: 0 };
    for (const entry of mine) {
      if (entry.what === "route") { await entry.handle?.unroute?.(); counts.routes += 1; }
      else if (entry.what === "use") { await entry.handle?.unroute?.(); counts.scenarios += 1; }
      else { await entry.driver?.uninstall?.(); counts.clocks += 1; }
    }
    return counts;
  };
  const gate = (target, path) => new Proxy(target, {
    get(object, prop) {
      const value = Reflect.get(object, prop, object);
      if (typeof prop !== "string") return value;
      if (typeof value === "function") {
        return (...args) => {
          if (state.revoked !== undefined) throw refuse(`${path}${prop}`);
          const install = options.attempts && INSTALLS.has(prop);
          if (install && state.opened && state.open === undefined) {
            throw new Error("no spec is running: the run refuses installs between specs");
          }
          const result = value.apply(object, args);
          if (install) {
            const entry = { attempt: state.open, what: prop, driver: object };
            state.installs.push(entry);
            if (result && typeof result.then === "function") return result.then((handle) => { entry.handle = handle; return handle; });
            entry.handle = result;
          }
          return result && typeof result === "object" && typeof result.then !== "function" ? gate(result, `${path}${prop}().`) : result;
        };
      }
      return value && typeof value === "object" ? gate(value, `${path}${prop}.`) : value;
    },
  });
  const functions = options.attempts ? {
    beginAttempt() {
      if (state.revoked !== undefined) throw new Error(`revoked: ${state.revoked}`);
      if (state.open !== undefined) throw new Error("an attempt is already open");
      state.next += 1;
      state.open = state.next;
      state.opened = true;
      return state.open;
    },
    async endAttempt(token) {
      if (state.open !== token) throw new Error(`attempt ${token} is not open`);
      state.open = undefined;
      if (state.failSweep) {
        const error = state.failSweep;
        state.failSweep = undefined;
        throw error;
      }
      return sweep(token);
    },
    async revoke(reason) {
      state.revoked = reason;
      const token = state.open;
      state.open = undefined;
      return token === undefined ? { routes: 0, scenarios: 0, clocks: 0, droppedTimers: 0 } : sweep(token);
    },
  } : {};
  return { state, gate, functions };
}

export function installFakeHost(world, options = {}) {
  const events = [];
  const attachments = new Map();
  const args = { ...(options.args ?? {}) };
  // lxdev's run controls (grep, ids, shard, retries, …), apart from args.
  const control = { ...(options.control ?? {}) };
  const logs = options.logs;
  const attempts = attemptHost(options);
  // `false`: the host has not registered its current lxapp yet (a runtime
  // that just reconnected); `lxapp()` without an id rejects as the host does.
  const current = { registered: options.current ?? true };

  globalThis.__LINGXIA_AUTOMATION_HOST__ = {
    args,
    control,
    async attach(name, artifact) {
      attachments.set(name, artifact);
    },
    emit(event) {
      events.push(event);
    },
    logs: logs === undefined
      ? undefined
      : async () => logs,
    ...attempts.functions,
  };

  globalThis.lx = {
    automation() {
      if (attempts.state.revoked !== undefined) {
        throw Object.assign(new Error(`E_AUTOMATION_PRIVILEGE: automation access of this run was revoked: ${attempts.state.revoked}`), {
          code: "E_AUTOMATION_PRIVILEGE",
        });
      }
      const root = {
        lxapp(appId) {
          if (!appId && !current.registered) throw new Error("no current lxapp");
          if (appId && options.apps?.[appId]) return options.apps[appId];
          return world.app;
        },
        ...(options.lxapps ? { lxapps: options.lxapps } : {}),
      };
      // `hostTiers`: the host's own shape — tiers are getters on the root's
      // prototype returning callable objects whose methods live on their
      // prototypes, as Rong host objects on JavaScriptCore are.
      if (options.hostTiers) {
        const proto = { lxapp: root.lxapp };
        for (const [name, tier] of Object.entries(options.hostTiers)) {
          Object.defineProperty(proto, name, { get() { return tier; }, enumerable: true, configurable: true });
        }
        return Object.create(proto);
      }
      if (!options.attempts) return root;
      return {
        lxapp: (appId) => attempts.gate(root.lxapp(appId), `lxapp(${appId ? JSON.stringify(appId) : ""}).`),
        ...(options.lxapps ? { lxapps: attempts.gate(options.lxapps, "lxapps.") } : {}),
      };
    },
  };

  return {
    events,
    attachments,
    args,
    control,
    /** The host's attempts: open token, installs, refused calls, revoke reason. */
    attempts: attempts.state,
    /** Register (or drop) the host's current lxapp. */
    setCurrent(registered) {
      current.registered = registered;
    },
  };
}
