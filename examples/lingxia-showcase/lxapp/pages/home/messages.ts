import type { DisplayLanguage } from "../../shared/display-language";

const enUS = {
  tagline: "Lightweight Application Framework",
  namePlaceholder: "Enter your name",
  sayHello: "Say Hello",
  sending: "Sending...",
  defaultGreeting: "This is from App's globalData.data",
  greetHello: "👋 Hello {name}! (#{count})",
  greetFrom: "🌍 Greetings from appservice powered by Rust and JS engine",
  appearance: "Appearance",
  appearanceAuto: "Auto",
  appearanceLight: "Light",
  appearanceDark: "Dark",
  appearanceUnavailable: "Appearance unavailable",
  language: "Language",
  languageAuto: "Auto",
  languageEn: "English",
  languageZh: "中文",
  languageUnavailable: "Language unavailable",
  myIp: "My IP",
} as const;

const catalogs = {
  "en-US": enUS,
  "zh-CN": {
    tagline: "轻量应用框架",
    namePlaceholder: "输入你的名字",
    sayHello: "打个招呼",
    sending: "发送中...",
    defaultGreeting: "来自 App 的 globalData.data",
    greetHello: "👋 你好，{name}！(#{count})",
    greetFrom: "🌍 来自 Rust 与 JS 引擎驱动的 App Service",
    appearance: "外观",
    appearanceAuto: "自动",
    appearanceLight: "浅色",
    appearanceDark: "深色",
    appearanceUnavailable: "无法切换外观",
    language: "语言",
    languageAuto: "自动",
    languageEn: "English",
    languageZh: "中文",
    languageUnavailable: "无法切换语言",
    myIp: "我的 IP",
  },
} satisfies Record<DisplayLanguage, Record<keyof typeof enUS, string>>;

export type MessageKey = keyof typeof enUS;

export function getMessages(displayLanguage: DisplayLanguage) {
  const catalog = catalogs[displayLanguage];
  return {
    t(key: MessageKey): string {
      return catalog[key];
    },
  };
}

export function formatHomeGreeting(
  displayLanguage: DisplayLanguage,
  name: string,
  count: number,
): string {
  const { t } = getMessages(displayLanguage);
  const time = new Date().toLocaleTimeString(displayLanguage, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
  return `${t("greetHello").replace("{name}", name).replace("{count}", String(count))}\n\n${t("greetFrom")}\n🕒 ${time}`;
}
