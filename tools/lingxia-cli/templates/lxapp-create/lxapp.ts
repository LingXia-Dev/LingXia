import { defineApp, lx } from '@lingxia/logic';

export default defineApp({
  onLaunch() {
    console.log('LingXia app launched');

    lx.app.update.onReady(async () => {
      const applyNow = await lx.ui.confirm({
        title: "Update Available",
        message: "A new version is ready. Apply now?",
        cancelText: "Later",
        confirmText: "Apply",
      });
      if (applyNow) {
        lx.app.update.apply();
      }
    });
    lx.app.update.onFailed((info) => {
      console.warn("Update failed:", info?.error);
    });
  },
});
