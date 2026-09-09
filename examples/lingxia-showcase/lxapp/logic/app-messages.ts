import {
  resolveDisplayLanguage,
  type DisplayLanguage,
} from "../shared/display-language";

/* Copy the App hands to native chrome — modals, more-actions, tab bar labels.
 * It lives beside App Logic because no View ever renders it, and nothing
 * re-renders it when the language changes. Page copy stays in that page's
 * `messages.ts`. */

const enUS = {
  tabHome: "Home",
  tabApi: "API",
  tabComponents: "Components",
  tabTodo: "ToDo",
  tabMedia: "Media",
  tabDevice: "Device",
  tabSurface: "Surface",
  moreFeedback: "Feedback",
  sidebarDownloads: "Downloads",
  sidebarChat: "Chat",
  sidebarTerminalSettings: "Terminal Settings",
  sidebarTerminal: "Terminal",
  sidebarPing: "Ping",
  pingToast: "sidebar action clicked",
  updateTitle: "Update Available",
  updateBody: "A new version is ready. Apply now?",
  updateLater: "Later",
  updateApply: "Apply",
} as const;

const catalogs = {
  "en-US": enUS,
  "zh-CN": {
    tabHome: "首页",
    tabApi: "接口",
    tabComponents: "组件",
    tabTodo: "待办",
    tabMedia: "媒体",
    tabDevice: "设备",
    tabSurface: "窗口",
    moreFeedback: "反馈",
    sidebarDownloads: "下载",
    sidebarChat: "聊天",
    sidebarTerminalSettings: "终端设置",
    sidebarTerminal: "终端",
    sidebarPing: "Ping",
    pingToast: "已点击侧栏操作",
    updateTitle: "有新版本",
    updateBody: "新版本已就绪，现在应用？",
    updateLater: "稍后",
    updateApply: "应用",
  },
} satisfies Record<DisplayLanguage, Record<keyof typeof enUS, string>>;

export type AppMessageKey = keyof typeof enUS;

export function getAppMessages(displayLanguage: DisplayLanguage) {
  const catalog = catalogs[displayLanguage];
  return {
    t(key: AppMessageKey): string {
      return catalog[key];
    },
  };
}

export async function applyShowcaseTabBar(
  tag = lx.app.displayLanguage.get(),
): Promise<void> {
  const { t } = getAppMessages(resolveDisplayLanguage(tag));
  await lx.tabBar.update({
    items: [
      { index: 0, text: t("tabHome") },
      { index: 1, text: t("tabApi") },
      { index: 2, text: t("tabComponents") },
      { index: 3, text: t("tabTodo") },
      { index: 4, text: t("tabMedia") },
      { index: 5, text: t("tabDevice") },
      { index: 6, text: t("tabSurface") },
    ],
  });
}
