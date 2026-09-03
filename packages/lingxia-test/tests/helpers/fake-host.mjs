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
  let currentPage = { name: "home", path: "pages/home/index", current: true, inStack: true, ready: true };
  const stack = [currentPage];
  let blocked = false;

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
    async click({ css, index }) {
      if (blocked) throw new Error("fixture should not reach the app after abort");
      const found = queryAll(css);
      const target = typeof index === "number" ? found[index] : found.find((element) => element.visible !== false);
      if (!target || target.visible === false) throw new Error(`click missed ${css}`);
      target.clicked = (target.clicked ?? 0) + 1;
      if (typeof target.onClick === "function") target.onClick(target);
    },
    async fill({ css, text, index }) {
      if (blocked) throw new Error("fixture should not reach the app after abort");
      const found = queryAll(css);
      const target = typeof index === "number" ? found[index] : found.find((element) => element.visible !== false);
      if (!target || target.visible === false) throw new Error(`fill missed ${css}`);
      target.value = text;
    },
    async type({ css, text }) {
      return page.fill({ css, text });
    },
    async screenshot() {
      return { format: "png", base64: TINY_PNG, width: 1, height: 1 };
    },
    async eval() {
      return null;
    },
  };

  const nav = {
    async relaunch({ page: name }) {
      if (blocked) throw new Error("fixture should not reach the app after abort");
      currentPage = { name, path: `pages/${name}/index`, current: true, inStack: true, ready: true };
      stack.splice(0, stack.length, currentPage);
      return currentPage;
    },
    async current() {
      return currentPage;
    },
    async to({ page: name }) {
      currentPage = { name, path: `pages/${name}/index`, current: true, inStack: true, ready: true };
      stack.push(currentPage);
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
      if (evalResults.has(script)) {
        value = evalResults.get(script);
        if (value instanceof Error) throw value;
      }
      // Mirror the runtime: with `captureCalls` the result is wrapped and
      // carries what the script reached. Tests seed that through `setCalls`.
      if (captureCalls) return { value, calls: evalCalls.get(script) ?? [] };
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
    unblock() {
      blocked = false;
    },
  };
}

export function installFakeHost(world, options = {}) {
  const events = [];
  const attachments = new Map();
  const args = { ...(options.args ?? {}) };
  const logs = options.logs;

  globalThis.__LINGXIA_AUTOMATION_HOST__ = {
    args,
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

  return { events, attachments, args };
}
