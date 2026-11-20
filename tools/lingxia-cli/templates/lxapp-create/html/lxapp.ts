App({
  onLaunch() {
    console.log('LingXia app launched');

    // Setup update manager
    const um = lx.getUpdateManager();
    um.onUpdateReady(async () => {
      const applyNow = await lx.ui.confirm({
        title: "Update Available",
        message: "A new version is ready. Apply now?",
        cancelText: "Later",
        confirmText: "Apply",
      });
      if (applyNow) {
        um.applyUpdate();
      }
    });
    um.onUpdateFailed((info) => {
      console.warn("Update failed:", info?.error);
    });
  },
  globalData: {
    greeting: 'Hello from App global data'
  }
});
