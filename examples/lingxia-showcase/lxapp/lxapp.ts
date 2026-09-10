import type { AppLaunchOptions, ConfiguredPageName } from "@lingxia/types";
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


const PRODUCT_LINK_PREFIX = "/showcase/";

function routeFromAppLink(options?: AppLaunchOptions) {
  if (options?.scene !== 8003) return;
  const pathname = options.url ? new URL(options.url).pathname : "";
  // Two shapes: a product path this app recognises, and `/lxapp/open?page=`.
  const page = pathname.startsWith(PRODUCT_LINK_PREFIX)
    ? pathname.slice(PRODUCT_LINK_PREFIX.length)
    : options.query?.page;
  if (!page) return;
  const query = { ...options.query };
  delete query.page;
  // A link carries whatever the sender wrote, so the name cannot be checked
  // here — navigateTo rejects one this lxapp does not have.
  void lx.navigateTo({ page: page as ConfiguredPageName, query });
}

async function applyHostChrome(os: string, tag: string) {
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
  return applyShowcaseTabBar(tag);
}

App({
  onLaunch: async function (this: ShowcaseAppInstance, options?: AppLaunchOptions) {
    routeFromAppLink(options);
    const { os } = lx.app.getBaseInfo();
    const applyChrome = (tag = lx.app.displayLanguage.get()) => {
      void applyHostChrome(os, tag).catch((error) =>
        console.warn("host chrome language update failed", error),
      );
    };
    // Preference clicks always move this event. Effective-tag `watch` can
    // miss a Harmony dispatch, which left tab labels on the previous language
    // while page copy and host "More" followed.
    lx.app.displayLanguage.watch((tag) => {
      applyChrome(tag);
    });
    lx.app.control?.displayLanguage.watchPreference(() => {
      applyChrome();
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

  onShow(options?: AppLaunchOptions) {
    routeFromAppLink(options);
    console.log("App.onShow");
    void applyShowcaseTabBar().catch((error) =>
      console.warn("tab bar language update failed", error),
    );
  },

  onUserCaptureScreen() {
    console.log("App.onUserCaptureScreen");
  },

  globalData: {
    greeting: "This is from App's globalData.data",
    ipAddr: "loading",
  },
});
