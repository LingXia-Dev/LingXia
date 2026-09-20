App({
  onLaunch() {
    console.log('LingXia app launched');

    // Setup update manager
    const um = lx.getUpdateManager();
    um.onUpdateReady(async () => {
      const applyNow = await lx.showModal({
        title: "Update Available",
        content: "A new version is ready. Apply now?",
        showCancel: true,
        cancelText: "Later",
        confirmText: "Apply",
      });
      if (applyNow.status !== 'canceled') {
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
