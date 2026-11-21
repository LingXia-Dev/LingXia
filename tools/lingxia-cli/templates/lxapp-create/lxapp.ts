import { lx } from '@lingxia/logic';
App({
  onLaunch() {
    console.log('LingXia app launched');

    lx.app.onUpdateReady(async () => {
      const applyNow = await lx.ui.confirm({
        title: "Update Available",
        message: "A new version is ready. Apply now?",
        cancelText: "Later",
        confirmText: "Apply",
      });
      if (applyNow) {
        lx.app.applyUpdate();
      }
    });
    lx.app.onUpdateFailed((info) => {
      console.warn("Update failed:", info?.error);
    });
  },
});
