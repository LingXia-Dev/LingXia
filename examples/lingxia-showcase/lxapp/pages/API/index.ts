import { cloudProviderAvailable } from "../../shared/lib/provider";
import { errorMessage } from "../../shared/lib/errors";
import { appState } from "../../shared/lib/app";
import { definePage } from '@lingxia/types/page';

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
        await lx.nav.go({ page: "stream" }).opened;
      },
      async navigateToChannelPage() {
        await lx.nav.go({ page: "channel" }).opened;
      },
      async navigateToBridgeReproPage() {
        await lx.nav.go({ page: "bridge-repro" }).opened;
      },
      // Navigate to UI API page with specific type parameter
      async navigateToUIPage(params: { type?: string } | undefined) {
        const { type = "navigation" } = params || {};
        await lx.nav.go({
          page: "ui",
          params: { type },
        }).opened;
      },
      // Navigate to Device API page with specific type parameter
      async navigateToDevicePage(params: { type?: string } | undefined) {
        const { type = "device" } = params || {};
        await lx.nav.go({
          page: "device",
          params: { type },
        }).opened;
      },
      // Navigate to WiFi API page
      async navigateToWifiPage() {
        await lx.nav.go({
          page: "wifi",
        }).opened;
      },
      // Navigate to System API page with specific type parameter
      async navigateToSystemPage(params: { type?: string } | undefined) {
        const { type = "appBaseInfo" } = params || {};
        await lx.nav.go({
          page: "system",
          params: { type },
        }).opened;
      },
      // Navigate to Location API page
      async navigateToLocationPage() {
        await lx.nav.go({
          page: "location",
        }).opened;
      },
      // Navigate to Media API page with specific type parameter
      async navigateToMediaPage(params: { type?: string } | undefined) {
        const { type = "Pictures" } = params || {};
        await lx.nav.go({
          page: "media",
          params: { type },
        }).opened;
      },
      async navigateToOpenFilePage() {
        await lx.nav.go({
          page: "file",
          params: { section: "openFile" },
        }).opened;
      },
      async navigateToChooseFilePage() {
        await lx.nav.go({
          page: "file",
          params: { section: "chooseFile" },
        }).opened;
      },
      async navigateToTestMiniApp() {
        try {
          await lx.nav.go({
            app: "lingxia-showcase-web",
            page: "chat",
          });
        } catch (err) {
          console.error("navigateToApp failed", err);
          lx.ui.toast({ title: errorMessage(err, "navigateToApp failed"), icon: "none" });
        }
      },
      async navigateToCloudPage(params: { type?: string } | undefined) {
        const { type = "auth" } = params || {};
        await lx.nav.go({
          page: "cloud",
          params: { type },
        }).opened;
      },
      async openDeepSeek() {
        const targets: Array<"self" | "aside" | "external"> = ["self", "aside", "external"];

        const choice = await lx.ui.actionSheet({
          items: targets.map((id) => ({ id, label: id })),
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
          lx.ui.toast({ title: errorMessage(error, "Request failed"), icon: "none" });
        }
      },
      async navigateToSharePage() {
        await lx.nav.go({
          page: "share",
        }).opened;
      },
      async navigateToClipboardPage() {
        await lx.nav.go({
          page: "clipboard",
        }).opened;
      },
      async navigateToLeaveGuardPage() {
        await lx.nav.go({ page: "leaveGuard" }).opened;
      },
      async exitApp() {
        const exit = await lx.ui.confirm({
          title: "Exit App",
          message: "lx.host.exit() exits immediately. Close the host app now?",
          tone: "danger",
          confirmText: "Exit",
          cancelText: "Cancel",
        });
        if (exit) {
          lx.host.exit();
        }
      },
      // Navigate to PullDownRefresh API page
      async navigateToPullDownRefreshPage() {
        await lx.nav.go({
          page: "pullToRefresh",
        }).opened;
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
        console.log("App data:", appState);
      },
      onHide() {
        console.log("API page onHide");
      },
      actions,
    };
  },
});
