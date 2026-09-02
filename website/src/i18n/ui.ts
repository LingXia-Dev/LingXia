export type Lang = 'en' | 'zh';

export const LANGS: Lang[] = ['en', 'zh'];
export const DEFAULT_LANG: Lang = 'en';

/**
 * All human-facing copy lives here, keyed by language.
 * Components receive `t = ui[lang]` and read from it — no hardcoded strings in markup.
 */
export const ui = {
  en: {
    meta: {
      title: 'LingXia — One runtime. Every platform.',
      description:
        'LingXia is a Rust-powered cross-platform app runtime. Build page-based lxapps and native host apps for Android, iOS, macOS, Windows, and HarmonyOS — with a clean View / Logic / Bridge split and a first-class CLI.',
    },
    nav: {
      architecture: 'Architecture',
      features: 'Features',
      development: 'Dev workflow',
      build: 'What you build',
      ai: 'AI-native',
      docs: 'Docs',
      github: 'GitHub',
      getStarted: 'Get started',
      menu: 'Menu',
    },
    hero: {
      eyebrow: 'Cross-platform app runtime',
      titleA: 'Write once.',
      titleB: 'Run native',
      titleC: 'on every platform.',
      subtitle:
        'LingXia is a Rust-powered runtime for building page-based lxapps and native host apps. One codebase ships to Android, iOS, macOS, Windows, and HarmonyOS — with UI rendering and business logic kept cleanly apart.',
      installLabel: 'Install the CLI',
      ctaPrimary: 'Get started',
      ctaSecondary: 'Star on GitHub',
      runtimeNote: 'MIT licensed · Rust runtime · React, Vue, or HTML views',
      copy: 'Copy',
      copied: 'Copied',
    },
    platforms: {
      label: 'One codebase, five native targets',
    },
    arch: {
      eyebrow: 'The architecture',
      title: 'A clean split between rendering and logic.',
      subtitle:
        'An lxapp is a page-based mini-app with a strict boundary. The View renders. The Logic owns state and platform APIs. A Rust bridge moves data and events between them — so UI work never tangles with business work.',
      layers: [
        {
          tag: 'View',
          runs: 'Runs in WebView',
          owns: 'Owns rendering',
          desc: 'React, Vue, or plain HTML. Renders replicated data and dispatches typed actions.',
        },
        {
          tag: 'Bridge',
          runs: 'Rust runtime',
          owns: 'Moves data & events',
          desc: 'setData, streams, channels, and native calls. The typed seam between the two worlds.',
        },
        {
          tag: 'Logic',
          runs: 'JS runtime or Rust',
          owns: 'Owns state & APIs',
          desc: 'Durable business state and portable lx.* APIs in JavaScript, with Rust for host-native power.',
        },
      ],
      footnote: 'View code renders · Logic code owns state · the bridge carries the rest',
    },
    features: {
      eyebrow: 'Why LingXia',
      title: 'Built for shipping real apps, fast.',
      subtitle: 'The runtime handles the cross-platform plumbing so you stay in product code.',
      items: [
        {
          title: 'Five platforms, one codebase',
          desc: 'Android, iOS, macOS, Windows, and HarmonyOS from a single project. Target one or all of them.',
          icon: 'platforms',
        },
        {
          title: 'Rust-powered runtime',
          desc: 'The loader, bridge, and platform abstraction are Rust — fast, memory-safe, and predictable.',
          icon: 'rust',
        },
        {
          title: 'Bring your framework',
          desc: 'Write Views in React, Vue, or HTML. Typed bindings and native components for each.',
          icon: 'framework',
        },
        {
          title: 'View / Logic separation',
          desc: 'Rendering and business logic live apart. State lives in Logic; the View just draws it.',
          icon: 'split',
        },
        {
          title: 'A CLI that does the work',
          desc: 'new, dev, doctor, build, publish. Scaffolds projects and runs them on real devices.',
          icon: 'cli',
        },
        {
          title: 'Native Rust extensions',
          desc: '#[lingxia::native] routes and HostAddon give you host APIs, background services, and native logic.',
          icon: 'extend',
        },
      ],
    },
    shapes: {
      eyebrow: 'What you build',
      title: 'Two project shapes. Native power when you need it.',
      subtitle: 'Start with an lxapp or a native host, then extend the host with Rust without changing the View model.',
      items: [
        {
          n: '01',
          tag: 'Standalone lxapp',
          desc: 'A page-based mini-app that runs inside any LingXia host. Perfect for pure UI and page work.',
          cmd: 'lingxia new my-lxapp -t lxapp -y',
        },
        {
          n: '02',
          tag: 'Native host app',
          desc: 'An installable Android / iOS / macOS / Windows / Harmony app embedding one or more lxapps. Most products.',
          cmd: 'lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y',
        },
        {
          n: '03',
          tag: 'Extend either with Rust',
          desc: 'Add host APIs, background services, native media, or Rust-owned logic inside a native host app.',
          cmd: '#[lingxia::native] + HostAddon',
        },
      ],
    },
    code: {
      eyebrow: 'Zero to running',
      title: 'From install to a running app in minutes.',
      subtitle: 'Install the CLI, scaffold a project, and run it. The same flow on every platform.',
      tabs: [
        { id: 'lxapp', label: 'Standalone lxapp' },
        { id: 'native', label: 'Native host app' },
        { id: 'skill', label: 'AI skill' },
      ],
      note: 'lingxia dev owns the live session; lxdev reloads, inspects, automates, and tests it.',
    },
    devtools: {
      eyebrow: 'Live development & automation',
      title: 'Build it. Drive it. Prove it.',
      subtitle:
        'One command owns the live app session. The other turns that session into a programmable development surface for people, scripts, and coding agents.',
      owner: {
        label: 'Session owner',
        title: 'lingxia dev',
        desc: 'Builds, installs, launches, and keeps the authenticated development websocket alive. Re-running it takes over the same project and platform cleanly.',
        command: 'lingxia dev --background',
        bullets: ['Native host or standalone lxapp', 'Real devices, simulators, and desktop Runner', 'Parallel sessions across different platforms'],
      },
      driver: {
        label: 'Session driver',
        title: 'lxdev',
        desc: 'Connects to the running session to reload lxapps, automate the UI, inspect both JavaScript contexts, run tests, capture screenshots, and stream every layer of logs.',
        command: 'lxdev lxapp reload',
        bullets: ['Browser, host window, and lxapp page automation', 'Logic eval and page DOM eval', 'Repeatable tests, screenshots, and filtered logs'],
      },
      loopLabel: 'The closed loop',
      loop: ['Edit', 'Reload or take over', 'Navigate & interact', 'Assert DOM or Logic', 'Check logs'],
      familiesLabel: 'Eight focused command families',
      families: [
        { name: 'lxapp', desc: 'lifecycle · navigation · pages · eval' },
        { name: 'app', desc: 'windows · native input · screenshots' },
        { name: 'desktop', desc: 'windows · accessibility · pointer · keyboard · clipboard' },
        { name: 'browser', desc: 'tabs · DOM · cookies · web automation' },
        { name: 'test', desc: 'API · page · end-to-end flows' },
        { name: 'logs', desc: 'native · View · Logic · browser' },
        { name: 'runner', desc: 'simulated device and frame for the desktop Runner' },
        { name: 'session', desc: 'discover and select live targets' },
      ],
      cta: 'Explore the development workflow',
    },
    ai: {
      eyebrow: 'AI-native by design',
      title: 'Ships with a skill your coding agent can read.',
      subtitle:
        'LingXia distributes its full reference as a portable markdown skill: the decision tree, every CLI command, page recipes, the lx.* API map, native components, and the Rust surface — laid out so an agent routes to the right section instead of reading everything.',
      bullets: [
        'Decision tree for lxapp vs. host app vs. Rust logic',
        'Complete CLI reference, page authoring, and lx.* API map',
        'Native component docs and the Rust native development guide',
        'Agent control for shipped products: appUse, computerUse, browserUse',
      ],
      installLabel: 'Install the skill',
      tools: 'Claude Code · OpenAI Codex CLI · Cursor · any markdown-reading agent',
      codexLabel: 'For Codex-style tools (writes AGENTS.md)',
    },
    packages: {
      eyebrow: 'The typed surface',
      title: 'Packages and generated imports for every layer.',
      subtitle: 'Use the framework binding for your View, global types for Logic, and the generated client for host routes.',
      items: [
        { name: '@lingxia/react', desc: 'React hooks + native component wrappers for Views' },
        { name: '@lingxia/vue', desc: 'Vue composables + native components, same surface' },
        { name: '@lingxia/html', desc: 'DOM helpers for HTML-only Views' },
        { name: '@lingxia/types', desc: 'TypeScript declarations for Page({}), App({}), lx.*' },
        { name: '@lingxia/bridge', desc: 'Low-level bridge runtime and invocation helpers' },
        { name: '@lingxia/elements', desc: 'Custom elements behind the framework wrappers' },
        { name: '@lingxia/test', desc: 'Authoring SDK for cases lxdev test runs against a live app' },
        { name: '@lingxia/native', desc: 'CLI-generated client for your host Rust routes' },
      ],
    },
    cta: {
      title: 'Build your first lxapp today.',
      subtitle: 'One command to install, one to create, one to run.',
      installLabel: 'Install the CLI',
      ctaPrimary: 'Read the quick start',
      ctaSecondary: 'Browse the source',
    },
    footer: {
      tagline: 'A Rust-powered cross-platform app runtime for lxapps, native shells, and Rust extensions.',
      colProduct: 'Product',
      colDocs: 'Docs',
      colCommunity: 'Community',
      links: {
        architecture: 'Architecture',
        features: 'Features',
        build: 'What you build',
        ai: 'AI-native',
        quickStart: 'Quick start',
        cliRef: 'CLI reference',
        skill: 'AI skill',
        lxappGuide: 'LxApp guide',
        github: 'GitHub',
        issues: 'Issues',
        license: 'MIT License',
      },
      rights: 'Released under the MIT License.',
    },
  },

  zh: {
    meta: {
      title: 'LingXia 灵匣 — 一次编写，五端原生运行',
      description:
        'LingXia（灵匣）是一个由 Rust 驱动的跨平台应用运行时。用一套代码构建页面式 lxapp 和原生宿主应用，覆盖 Android、iOS、macOS、Windows 与 HarmonyOS——视图渲染与业务逻辑彻底分离，配套一流的命令行工具。',
    },
    nav: {
      architecture: '架构',
      features: '特性',
      development: '开发闭环',
      build: '构建形态',
      ai: 'AI 原生',
      docs: '文档',
      github: 'GitHub',
      getStarted: '开始使用',
      menu: '菜单',
    },
    hero: {
      eyebrow: '跨平台应用运行时',
      titleA: '一次编写，',
      titleB: '五端原生',
      titleC: '运行。',
      subtitle:
        'LingXia 是由 Rust 驱动的运行时，用于构建页面式 lxapp 和原生宿主应用。一套代码即可交付到 Android、iOS、macOS、Windows 与 HarmonyOS——界面渲染与业务逻辑彻底分离。',
      installLabel: '安装命令行工具',
      ctaPrimary: '开始使用',
      ctaSecondary: '在 GitHub 加星',
      runtimeNote: 'MIT 许可 · Rust 运行时 · React / Vue / HTML 视图',
      copy: '复制',
      copied: '已复制',
    },
    platforms: {
      label: '一套代码，五个原生目标',
    },
    arch: {
      eyebrow: '核心架构',
      title: '渲染与逻辑，干净分离。',
      subtitle:
        'lxapp 是页面式小应用，边界严格：视图负责渲染，逻辑掌管状态与平台 API，一条 Rust 桥在两者之间传递数据与事件——界面工作不会与业务工作纠缠在一起。',
      layers: [
        {
          tag: 'View 视图',
          runs: '运行于 WebView',
          owns: '负责渲染',
          desc: 'React、Vue 或纯 HTML。渲染复制而来的数据，并派发类型化 action。',
        },
        {
          tag: 'Bridge 桥',
          runs: 'Rust 运行时',
          owns: '传递数据与事件',
          desc: 'setData、流、通道与原生调用。连接两端的类型化接缝。',
        },
        {
          tag: 'Logic 逻辑',
          runs: 'JS 运行时或 Rust',
          owns: '掌管状态与 API',
          desc: 'JavaScript 掌管持久业务状态与可移植 lx.* API，Rust 提供宿主原生能力。',
        },
      ],
      footnote: '视图只渲染 · 逻辑掌状态 · 其余交给桥',
    },
    features: {
      eyebrow: '为什么是 LingXia',
      title: '为快速交付真实应用而生。',
      subtitle: '运行时处理跨平台的繁琐管线，你只需专注产品代码。',
      items: [
        {
          title: '五端，一套代码',
          desc: 'Android、iOS、macOS、Windows、HarmonyOS 来自同一项目。可只做其一，也可全覆盖。',
          icon: 'platforms',
        },
        {
          title: 'Rust 驱动的运行时',
          desc: '加载器、桥与平台抽象层皆由 Rust 写成——快速、内存安全、行为可预期。',
          icon: 'rust',
        },
        {
          title: '沿用你的框架',
          desc: '用 React、Vue 或 HTML 写视图。每种都配有类型化绑定与原生组件。',
          icon: 'framework',
        },
        {
          title: '视图与逻辑分离',
          desc: '渲染与业务逻辑各居其位。状态归逻辑，视图只负责把它画出来。',
          icon: 'split',
        },
        {
          title: '能干活的命令行',
          desc: 'new、dev、doctor、build、publish。一键脚手架，并在真机上运行。',
          icon: 'cli',
        },
        {
          title: '原生 Rust 扩展',
          desc: '#[lingxia::native] 路由与 HostAddon 提供宿主 API、后台服务与原生逻辑。',
          icon: 'extend',
        },
      ],
    },
    shapes: {
      eyebrow: '你能构建什么',
      title: '两种项目形态，需要时再接入原生能力。',
      subtitle: '从独立 lxapp 或原生宿主开始，再用 Rust 扩展宿主，不改变 View 的开发模型。',
      items: [
        {
          n: '01',
          tag: '独立 lxapp',
          desc: '可在任意 LingXia 宿主中运行的页面式小应用。最适合纯界面与页面开发。',
          cmd: 'lingxia new my-lxapp -t lxapp -y',
        },
        {
          n: '02',
          tag: '原生宿主应用',
          desc: '可安装的 Android / iOS / macOS / Windows / Harmony 应用，内嵌一个或多个 lxapp。多数产品的选择。',
          cmd: 'lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y',
        },
        {
          n: '03',
          tag: '用 Rust 扩展宿主',
          desc: '在原生宿主内添加宿主 API、后台服务、原生媒体或由 Rust 掌管的逻辑。',
          cmd: '#[lingxia::native] + HostAddon',
        },
      ],
    },
    code: {
      eyebrow: '从零到运行',
      title: '从安装到跑起来，只需几分钟。',
      subtitle: '安装命令行、生成项目、直接运行。每个平台都是同样的流程。',
      tabs: [
        { id: 'lxapp', label: '独立 lxapp' },
        { id: 'native', label: '原生宿主应用' },
        { id: 'skill', label: 'AI 技能' },
      ],
      note: 'lingxia dev 负责实时会话；lxdev 负责重载、检查、自动化与测试。',
    },
    devtools: {
      eyebrow: '实时开发与自动化',
      title: '构建、驱动、验证，形成闭环。',
      subtitle:
        '一条命令拥有实时应用会话，另一条把它变成可编程的开发界面，供开发者、脚本与编码 Agent 共同使用。',
      owner: {
        label: '会话拥有者',
        title: 'lingxia dev',
        desc: '负责构建、安装、启动，并维持经过认证的开发 WebSocket。再次运行会干净地接管同一项目、同一平台的旧会话。',
        command: 'lingxia dev --background',
        bullets: ['支持原生宿主与独立 lxapp', '覆盖真机、模拟器与桌面 Runner', '不同平台会话可同时运行'],
      },
      driver: {
        label: '会话驱动器',
        title: 'lxdev',
        desc: '连接运行中的会话，重载 lxapp、自动化 UI、检查两个 JavaScript context、运行测试、截图，并汇总每一层日志。',
        command: 'lxdev lxapp reload',
        bullets: ['浏览器、宿主窗口与 lxapp 页面自动化', 'Logic eval 与页面 DOM eval', '可重复测试、截图与日志过滤'],
      },
      loopLabel: '完整闭环',
      loop: ['编辑', '重载或接管', '导航并交互', '断言 DOM 或 Logic', '检查日志'],
      familiesLabel: '八个聚焦的命令家族',
      families: [
        { name: 'lxapp', desc: '生命周期 · 导航 · 页面 · eval' },
        { name: 'app', desc: '窗口 · 原生输入 · 截图' },
        { name: 'desktop', desc: '窗口 · 无障碍树 · 指针 · 键盘 · 剪贴板' },
        { name: 'browser', desc: '标签 · DOM · Cookie · Web 自动化' },
        { name: 'test', desc: 'API · 页面 · 端到端流程' },
        { name: 'logs', desc: '原生 · View · Logic · 浏览器' },
        { name: 'runner', desc: '桌面 Runner 的模拟设备与外框' },
        { name: 'session', desc: '发现并选择实时目标' },
      ],
      cta: '查看完整开发工作流',
    },
    ai: {
      eyebrow: '原生面向 AI',
      title: '内置一份你的编码助手能读懂的技能。',
      subtitle:
        'LingXia 把完整参考资料做成一份可移植的 markdown 技能：决策树、每条 CLI 命令、页面写法、lx.* API 地图、原生组件、Rust 接口——经过编排，让 AI 助手直接路由到所需章节，而非通读全部。',
      bullets: [
        'lxapp / 宿主应用 / Rust 逻辑 的决策树',
        '完整 CLI 参考、页面编写与 lx.* API 地图',
        '原生组件文档与 Rust 原生开发指南',
        '交付产品的 agent 控制面：appUse、computerUse、browserUse',
      ],
      installLabel: '安装技能',
      tools: 'Claude Code · OpenAI Codex CLI · Cursor · 任意读 markdown 的助手',
      codexLabel: '面向 Codex 类工具（会写入 AGENTS.md）',
    },
    packages: {
      eyebrow: '类型化接口',
      title: '每一层都有对应的包或生成入口。',
      subtitle: 'View 使用框架绑定，Logic 使用全局类型，宿主路由使用 CLI 生成的客户端。',
      items: [
        { name: '@lingxia/react', desc: 'React Hooks + 视图用原生组件封装' },
        { name: '@lingxia/vue', desc: 'Vue 组合式 API + 原生组件，接口一致' },
        { name: '@lingxia/html', desc: '纯 HTML 视图的 DOM 辅助函数' },
        { name: '@lingxia/types', desc: 'Page({})、App({})、lx.* 的 TypeScript 声明' },
        { name: '@lingxia/bridge', desc: '底层桥运行时与调用辅助' },
        { name: '@lingxia/elements', desc: '框架封装背后的自定义元素' },
        { name: '@lingxia/test', desc: '编写用例的 SDK，由 lxdev test 对真实应用执行' },
        { name: '@lingxia/native', desc: 'CLI 为宿主 Rust 路由生成的客户端' },
      ],
    },
    cta: {
      title: '今天就构建你的第一个 lxapp。',
      subtitle: '一条命令安装，一条命令创建，一条命令运行。',
      installLabel: '安装命令行工具',
      ctaPrimary: '阅读快速开始',
      ctaSecondary: '浏览源码',
    },
    footer: {
      tagline: '由 Rust 驱动的跨平台应用运行时，承载 lxapp、原生外壳与 Rust 扩展。',
      colProduct: '产品',
      colDocs: '文档',
      colCommunity: '社区',
      links: {
        architecture: '架构',
        features: '特性',
        build: '构建形态',
        ai: 'AI 原生',
        quickStart: '快速开始',
        cliRef: 'CLI 参考',
        skill: 'AI 技能',
        lxappGuide: 'LxApp 指南',
        github: 'GitHub',
        issues: '问题反馈',
        license: 'MIT 许可证',
      },
      rights: '基于 MIT 许可证发布。',
    },
  },
} as const;

export type T = (typeof ui)[Lang];

export function getT(lang: Lang): T {
  return ui[lang];
}

/** The home path for a language (used by the language switch). */
export function langHome(lang: Lang): string {
  return lang === DEFAULT_LANG ? '/' : `/${lang}/`;
}
