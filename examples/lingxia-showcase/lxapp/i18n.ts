/** Showcase catalogs. Narrow the product's tag; this is not a second language setting. */

export type ShowcaseCatalog = "en-US" | "zh-CN";

export function showcaseCatalog(tag: string): ShowcaseCatalog {
  const primary = tag.replace("_", "-").split("-")[0]?.toLowerCase();
  return primary === "zh" ? "zh-CN" : "en-US";
}

const TAB_ITEMS = [
  { index: 0, "en-US": "Home", "zh-CN": "首页" },
  { index: 1, "en-US": "API", "zh-CN": "接口" },
  { index: 2, "en-US": "Components", "zh-CN": "组件" },
  { index: 3, "en-US": "ToDo", "zh-CN": "待办" },
  { index: 4, "en-US": "Media", "zh-CN": "媒体" },
  { index: 5, "en-US": "Device", "zh-CN": "设备" },
  { index: 6, "en-US": "Surface", "zh-CN": "窗口" },
] as const;

export function tabBarItems(tag: string): Array<{ index: number; text: string }> {
  const catalog = showcaseCatalog(tag);
  return TAB_ITEMS.map((item) => ({ index: item.index, text: item[catalog] }));
}

export async function applyShowcaseTabBar(tag = lx.app.displayLanguage.get()): Promise<void> {
  await lx.tabBar.update({ items: tabBarItems(tag) });
}

export type HomeCopy = {
  tagline: string;
  namePlaceholder: string;
  sayHello: string;
  sending: string;
  defaultGreeting: string;
  appearance: string;
  appearanceAuto: string;
  appearanceLight: string;
  appearanceDark: string;
  myIp: string;
  appearanceUnavailable: string;
};

const HOME_COPY: Record<ShowcaseCatalog, HomeCopy> = {
  "en-US": {
    tagline: "Lightweight Application Framework",
    namePlaceholder: "Enter your name",
    sayHello: "Say Hello",
    sending: "Sending...",
    defaultGreeting: "This is from App's globalData.data",
    appearance: "Appearance",
    appearanceAuto: "Auto",
    appearanceLight: "Light",
    appearanceDark: "Dark",
    myIp: "My IP",
    appearanceUnavailable: "Appearance unavailable",
  },
  "zh-CN": {
    tagline: "轻量应用框架",
    namePlaceholder: "输入你的名字",
    sayHello: "打个招呼",
    sending: "发送中...",
    defaultGreeting: "来自 App 的 globalData.data",
    appearance: "外观",
    appearanceAuto: "自动",
    appearanceLight: "浅色",
    appearanceDark: "深色",
    myIp: "我的 IP",
    appearanceUnavailable: "无法切换外观",
  },
};

export function homeCopy(tag = lx.app.displayLanguage.get()): HomeCopy {
  return HOME_COPY[showcaseCatalog(tag)];
}

export function homeGreeting(name: string, count: number, tag = lx.app.displayLanguage.get()): string {
  const time = new Date().toLocaleTimeString(showcaseCatalog(tag), {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  if (showcaseCatalog(tag) === "zh-CN") {
    return `👋 你好，${name}！(#${count})\n\n🌍 来自 Rust 与 JS 引擎驱动的 App Service\n🕒 ${time}`;
  }
  return `👋 Hello ${name}! (#${count})\n\n🌍 Greetings from appservice powered by Rust and JS engine\n🕒 ${time}`;
}
