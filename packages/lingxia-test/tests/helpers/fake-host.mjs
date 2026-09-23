const TINY_PNG =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

function testIdFromCss(css) {
  const match = String(css).match(/\[data-testid=["']?([^"'\]]+)/);
  return match ? match[1].replace(/\\/g, "") : undefined;
}

export function createWorld(options = {}) {
  const elements = options.elements ? [...options.elements] : [];
  const evalResults = new Map();
  const evalCalls = new Map();
  const landedPage = (name) =>
    ({ name, path: `pages/${name}/index`, current: true, inStack: true, ready: true, webviewAttached: true });
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
    async eval({ script }) {
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

  const app = {
    page,
    nav,
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
    async eval({ script, captureCalls }) {
      if (blocked) throw new Error("fixture should not reach the app after abort");
      let value = script;
      if (logicGlobals && !evalResults.has(script)) value = await evaluate(logicGlobals, script);
      if (evalResults.has(script)) {
        value = evalResults.get(script);
        if (value instanceof Error) throw value;
      }
      // Mirror the runtime: with `captureCalls` the result is wrapped and
      // carries what the script reached. Tests seed that through `setCalls`.
      if (captureCalls) {
        // Mirror the runtime: `value` is absent when the script returns undefined.
        const envelope = { __lxEval: 1, calls: evalCalls.get(script) ?? [] };
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
      in_viewport: element.visible !== false && element.inViewport !== false,
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
    unblock() {
      blocked = false;
    },
  };
}

export function installFakeHost(world, options = {}) {
  const events = [];
  const attachments = new Map();
  const args = { ...(options.args ?? {}) };
  // Omitted, the host predates the control channel and controls ride in args.
  const control = options.control === undefined ? undefined : { ...options.control };
  const logs = options.logs;

  globalThis.__LINGXIA_AUTOMATION_HOST__ = {
    args,
    ...(control ? { control } : {}),
    async attach(name, artifact) {
      attachments.set(name, artifact);
    },
    emit(event) {
      events.push(event);
    },
    logs: logs === undefined
      ? undefined
      : async () => logs,
  };

  globalThis.lx = {
    automation() {
      return {
        lxapp(appId) {
          if (appId && options.apps?.[appId]) return options.apps[appId];
          return world.app;
        },
      };
    },
  };

  return { events, attachments, args, control };
}
