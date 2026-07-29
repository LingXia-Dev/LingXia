async function testFileManagerAccess() {
  try {
    const files = lx.getFileManager();
    const filePath = "debug/app-launch.txt";
    await files.mkdir({ path: "debug", recursive: true });
    await files.writeFile({
      filePath,
      data: `FileManager test created at ${new Date().toISOString()}`,
      overwrite: true,
    });
    const { data } = await files.readFile({ filePath, encoding: "utf8" });
    console.log("[FileManager Test] Content:", data);
  } catch (error) {
    console.warn("[FileManager Test] Error:", (error as Error).message);
  }
}

interface MyAppInstance {
  globalData: {
    greeting: string;
    ipAddr: string;
  };
  ipReadyCallback?: (ip: string) => void;
}

App({
  onLaunch: async function (this: MyAppInstance) {
    const { os } = lx.app.getBaseInfo();
    type SidebarAction = Parameters<typeof lx.shell.sidebarActions.replace>[0][number];
    const sidebarActions: SidebarAction[] = [
      {
        id: "downloads",
        placement: "header",
        icon: "public/activator.svg",
        label: "Downloads",
        onActivate: () => {
          void lx
            .openSurface({ url: "lingxia://downloads" })
            .catch((error) => console.warn("downloads action failed", error));
        },
      },
      {
        id: "settings",
        placement: "header",
        icon: "public/activator.svg",
        label: "Settings",
        onActivate: () => {
          void lx
            .openSurface({ url: "lingxia://settings" })
            .catch((error) => console.warn("settings action failed", error));
        },
      },
      {
        id: "chat",
        placement: "footer",
        icon: "public/activator.svg",
        label: "chat",
        onActivate: () => {
          void lx
            .openSurface({ lxapp: "lingxia-chat", as: "aside" })
            .catch((error) => console.warn("chat action failed", error));
        },
      },
    ];

    if (os === "macOS" || os === "Windows") {
      sidebarActions.push({
        id: "terminal",
        placement: "footer",
        icon: "public/activator.svg",
        label: "Terminal",
        onActivate: () => {
          void lx
            .openSurface({ native: "terminal" })
            .catch((error) => console.warn("terminal action failed", error));
        },
      });
    }

    sidebarActions.push(
      {
        id: "ping",
        placement: "footer",
        icon: "public/activator.svg",
        label: "Ping",
        onActivate: () => {
          lx.showToast({ title: "sidebar action clicked", icon: "success" });
        },
      },
    );
    lx.shell.sidebarActions.replace(sidebarActions);

    const um = lx.getUpdateManager();
    um.onUpdateReady(async (info) => {
      if (info?.isForceUpdate) {
        console.log("Force update ready; apply immediately");
        um.applyUpdate();
        return;
      }

      console.log("Update ready; asking user to apply...");
      const { confirm } = await lx.showModal({
        title: "Update Available",
        content: "A new version is ready. Apply now?",
        showCancel: true,
        cancelText: "Later",
        confirmText: "Apply",
      });
      if (confirm) {
        um.applyUpdate();
      }
    });
    um.onUpdateFailed((info) => {
      console.warn("Update failed", info);
    });

    testFileManagerAccess();

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

  onShow() {
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
