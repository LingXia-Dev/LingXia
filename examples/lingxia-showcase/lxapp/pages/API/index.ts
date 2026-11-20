import { cloudProviderAvailable } from "../../shared/lib/provider";
import { errorMessage } from "../../shared/lib/errors";
import { showcaseApp } from "../../shared/lib/app";
import { definePage } from '@lingxia/types/page';
const app = showcaseApp();

export default definePage({
  data: {
    // lx.cloud / lx.auth exist only when the host ships the cloud
    // provider; hide the Cloud section entirely otherwise.
    cloudAvailable: false,
    loadCount: 0,
  },
  setup(page) {
    const actions = {
      async navigateToStreamPage() {
        await lx.navigateTo({ page: "stream" });
      },
      async navigateToChannelPage() {
        await lx.navigateTo({ page: "channel" });
      },
      async navigateToBridgeReproPage() {
        await lx.navigateTo({ page: "bridge-repro" });
      },
      // Navigate to UI API page with specific type parameter
      async navigateToUIPage(params: { type?: string } | undefined) {
        const { type = "navigation" } = params || {};
        await lx.navigateTo({
          page: "ui",
          query: { type },
        });
      },
      // Navigate to Device API page with specific type parameter
      async navigateToDevicePage(params: { type?: string } | undefined) {
        const { type = "device" } = params || {};
        await lx.navigateTo({
          page: "device",
          query: { type },
        });
      },
      // Navigate to WiFi API page
      async navigateToWifiPage() {
        await lx.navigateTo({
          page: "wifi",
        });
      },
      // Navigate to System API page with specific type parameter
      async navigateToSystemPage(params: { type?: string } | undefined) {
        const { type = "appBaseInfo" } = params || {};
        await lx.navigateTo({
          page: "system",
          query: { type },
        });
      },
      // Navigate to Location API page
      async navigateToLocationPage() {
        await lx.navigateTo({
          page: "location",
        });
      },
      // Navigate to Media API page with specific type parameter
      async navigateToMediaPage(params: { type?: string } | undefined) {
        const { type = "Pictures" } = params || {};
        await lx.navigateTo({
          page: "media",
          query: { type },
        });
      },
      async navigateToOpenFilePage() {
        await lx.navigateTo({
          page: "file",
          query: { section: "openFile" },
        });
      },
      async navigateToChooseFilePage() {
        await lx.navigateTo({
          page: "file",
          query: { section: "chooseFile" },
        });
      },
      async navigateToTestMiniApp() {
        try {
          await lx.navigateToApp({
            appId: "lingxia-chat",
            page: "chat",
          });
        } catch (err) {
          console.error("navigateToApp failed", err);
          lx.showToast({ title: errorMessage(err, "navigateToApp failed"), icon: "none" });
        }
      },
      async navigateToCloudPage(params: { type?: string } | undefined) {
        const { type = "auth" } = params || {};
        await lx.navigateTo({
          page: "cloud",
          query: { type },
        });
      },
      async openDeepSeek() {
        const targets: Array<"self" | "aside" | "external"> = ["self", "aside", "external"];

        const choice = await lx.showActionSheet({
          items: targets.map((id) => ({ id, label: id })),
          itemColor: "#007AFF",
        });
        if (choice.status === 'canceled') {
          return;
        }

        try {
          const url = "https://www.deepseek.com/";
          if (choice.id === "external") {
            lx.openExternal(url);
          } else if (choice.id === "aside") {
            await lx.surface.openUrl(url, { as: "aside" });
          } else {
            await lx.surface.openUrl(url);
          }
        } catch (error) {
          lx.showToast({ title: errorMessage(error, "Request failed"), icon: "none" });
        }
      },
      async navigateToSharePage() {
        await lx.navigateTo({
          page: "share",
        });
      },
      async navigateToClipboardPage() {
        await lx.navigateTo({
          page: "clipboard",
        });
      },
      async navigateToLeaveGuardPage() {
        await lx.navigateTo({ page: "leaveGuard" });
      },
      async exitApp() {
        const result = await lx.showModal({
          title: "Exit App",
          content: "lx.host.exit() exits immediately. Close the host app now?",
          confirmText: "Exit",
          cancelText: "Cancel",
        });
        if (result.status !== 'canceled') {
          lx.host.exit();
        }
      },
      // Navigate to PullDownRefresh API page
      async navigateToPullDownRefreshPage() {
        await lx.navigateTo({
          page: "pullToRefresh",
        });
      },
    };
    return {
      onLoad(options) {
        console.log("Options in onLoad: ", options);
        page.update(d => {
          d.cloudAvailable = cloudProviderAvailable();
          d.loadCount = (page.data.loadCount || 0) + 1;
        });
      },
      onShow() {
        console.log("API page onShow");
        console.log("App data:", app.globalData);
      },
      onHide() {
        console.log("API page onHide");
      },
      actions,
    };
  },
});
