import type { DisplayLanguage } from "../shared/display-language";

/* Copy the App hands to native chrome — modals, more-actions, sidebar actions.
 * Tab labels and page titles are not here: lxapp.json declares them per
 * language and the host follows the display language on its own.
 * It lives beside App Logic because no View ever renders it, and nothing
 * re-renders it when the language changes. Page copy stays in that page's
 * `messages.ts`. */

const enUS = {
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
