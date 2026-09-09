import type { ConfiguredPageName } from "@lingxia/types";
import type { ShowcaseAppInstance } from "./shared/lib/app";
import { applyShowcaseTabBar, getAppMessages } from "./logic/app-messages";
import { resolveDisplayLanguage } from "./shared/display-language";


async function testManagedFileAccess() {
  try {
    const filePath = "debug/app-launch.txt";
    await lx.fs.mkdir("debug", { recursive: true });
    await lx.fs.write(filePath, `Managed file test created at ${new Date().toISOString()}`, {
      overwrite: true,
    });
    const data = await lx.fs.file(filePath).text();
    console.log("[Managed File Test] Content:", data);
  } catch (error) {
    console.warn("[Managed File Test] Error:", (error as Error).message);
  }
}


function routeFromAppLink(options?: { scene?: number; query?: Record<string, string> }) {
  if (options?.scene !== 8003) return;
  const page = options.query?.page;
  if (!page) return;
  const query = { ...options.query };
  delete query.page;
  // An AppLink carries whatever the caller wrote, so the name cannot be checked
  // here — navigateTo rejects one this lxapp does not have.
  void lx.navigateTo({ page: page as ConfiguredPageName, query });
}

function applyHostChrome(os: string, tag: string) {
  const { t } = getAppMessages(resolveDisplayLanguage(tag));
  type SidebarAction = Parameters<typeof lx.shell.sidebarActions.replace>[0][number];
  const sidebarActions: SidebarAction[] = [
    {
      id: "downloads",
      placement: "header",
      icon: "public/sidebar-downloads.svg",
      label: t("sidebarDownloads"),
      onActivate: () => {
        void lx.shell
          .openBuiltin("downloads")
          .catch((error) => console.warn("downloads action failed", error));
      },
    },
    {
      id: "chat",
      placement: "footer",
      icon: "public/activator.svg",
      label: t("sidebarChat"),
      onActivate: () => {
        void lx.surface
          .openDeclared("lingxia-chat")
          .catch((error) => console.warn("chat action failed", error));
      },
    },
  ];

  if (os === "macOS" || os === "Windows") {
    sidebarActions.push({
      id: "terminal-settings",
      placement: "footer",
      icon: "public/sidebar-terminal.svg",
      label: t("sidebarTerminalSettings"),
      onActivate: () => {
        void lx.shell
          .openApp("app.lingxia.terminal-settings", { as: "aside", edge: "right" })
          .catch((error) => console.warn("terminal settings action failed", error));
      },
    });
    sidebarActions.push({
      id: "terminal",
      placement: "footer",
      icon: "public/activator.svg",
      label: t("sidebarTerminal"),
      onActivate: () => {
        void lx.surface
          .openDeclared("terminal")
          .catch((error) => console.warn("terminal action failed", error));
      },
    });
  }

  sidebarActions.push({
    id: "ping",
    placement: "footer",
    icon: "public/activator.svg",
    label: t("sidebarPing"),
    onActivate: () => {
      lx.showToast({ title: t("pingToast"), icon: "success" });
    },
  });
  lx.shell.sidebarActions.replace(sidebarActions);
  lx.setMoreActions([
    {
      icon: "public/showcase-icon.png",
      label: t("moreFeedback"),
      onClick: async () => {
        try {
          await lx.surface.openPage("feedback", {
            as: "float",
            position: "bottom",
            size: { width: "100%", height: "80%" },
            interaction: {
              closeButton: true,
              dismiss: "manual",
              modal: true,
            },
          });
        } catch (error) {
          console.warn("failed to open feedback surface", error);
        }
      },
    },
  ]);
  void applyShowcaseTabBar(tag).catch((error) =>
    console.warn("tab bar language update failed", error),
  );
}

App({
  onLaunch: async function (
    this: ShowcaseAppInstance,
    options?: { scene?: number; query?: Record<string, string> },
  ) {
    routeFromAppLink(options);
    const { os } = lx.app.getBaseInfo();
    lx.app.displayLanguage.watch((tag) => {
      applyHostChrome(os, tag);
    });

    const um = lx.getUpdateManager();
    um.onUpdateReady(async (info) => {
      if (info?.isForceUpdate) {
        console.log("Force update ready; apply immediately");
        um.applyUpdate();
        return;
      }

      console.log("Update ready; asking user to apply...");
      const { t } = getAppMessages(resolveDisplayLanguage(lx.app.displayLanguage.get()));
      const applyNow = await lx.showModal({
        title: t("updateTitle"),
        content: t("updateBody"),
        showCancel: true,
        cancelText: t("updateLater"),
        confirmText: t("updateApply"),
      });
      if (!applyNow.canceled) {
        um.applyUpdate();
      }
    });
    um.onUpdateFailed((info) => {
      console.warn("Update failed", info);
    });

    testManagedFileAccess();

    try {
      const response = await fetch("https://api64.ipify.org?format=json");
      const data = (await response.json()) as { ip: string };
      this.globalData.ipAddr = data.ip;
      console.log("Got public address:", data.ip);
    } catch (error) {
      this.globalData.ipAddr = (error as Error).message;
    }

    // Call the registered callback function if available
    if (this.ipReadyCallback) {
      console.log("Calling IP ready callback");
      this.ipReadyCallback(this.globalData.ipAddr);
    }
  },

  onHide() {
    console.log("App.onHide");
  },

  onShow(options?: { scene?: number; query?: Record<string, string> }) {
    routeFromAppLink(options);
    console.log("App.onShow");
  },

  onUserCaptureScreen() {
    console.log("App.onUserCaptureScreen");
  },

  globalData: {
    greeting: "This is from App's globalData.data",
    ipAddr: "loading",
  },
});
