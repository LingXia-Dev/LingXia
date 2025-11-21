App({
  onLaunch: async function () {
    try {
      const response = await fetch("https://api64.ipify.org?format=json");
      const data = await response.json();
      this.globalData.ipAddr = data.ip;
      console.log("Got public address: ", data.ip);
    } catch (error) {
      this.globalData.ipAddr = error.message;
    }

    const um = lx.getUpdateManager();
    um.onUpdateReady(async () => {
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
    um.onUpdateFailed(() => {
      console.warn("Update failed");
    });

    // Call the registered callback function if available
    if (this.ipReadyCallback) {
      console.log("Calling IP ready callback");
      this.ipReadyCallback(this.globalData.ipAddr);
    }
  },

  onHide: function () {
    console.log("Lxapp is hidden");
  },

  onShow: function (options) {
    console.log("Options in App's onShow: ", options);
  },

  globalData: {
    greeting: "This is from App's globalData.data",
    ipAddr: "loading",
  },
});
