/**
 * Test cache file access - verifies atime update for LRU cleanup
 */
async function testCacheAccess() {
  const cachePath = `${lx.env.USER_CACHE_PATH}/cache_test.txt`;
  const testContent = `Cache test created at ${new Date().toISOString()}`;

  try {
    // Write a test file to cache directory
    await Rong.writeTextFile(cachePath, testContent);
    console.log("[Cache Test] Written test file:", cachePath);

    // Read the file back - this should trigger atime update
    const content = await Rong.readTextFile(cachePath);
    console.log("[Cache Test] Read test file, atime should be updated");
    console.log("[Cache Test] Content:", content);
  } catch (error) {
    console.warn("[Cache Test] Error:", (error as Error).message);
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
    // Test cache file access time update
    testCacheAccess();

    try {
      const response = await fetch("https://api64.ipify.org?format=json");
      const data = (await response.json()) as { ip: string };
      this.globalData.ipAddr = data.ip;
      console.log("Got public address:", data.ip);
    } catch (error) {
      this.globalData.ipAddr = (error as Error).message;
    }

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
