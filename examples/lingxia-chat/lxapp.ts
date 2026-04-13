App({
  onLaunch() {
    console.log('LingXia Chat launched');

    const um = lx.getUpdateManager();
    um.onUpdateReady(async (info) => {
      if (info?.isForceUpdate) {
        console.log('Force update ready; apply immediately');
        um.applyUpdate();
        return;
      }

      const { confirm } = await lx.showModal({
        title: 'Update Available',
        content: 'A new version is ready. Apply now?',
        showCancel: true,
        cancelText: 'Later',
        confirmText: 'Apply',
      });
      if (confirm) {
        um.applyUpdate();
      }
    });
    um.onUpdateFailed((info) => {
      console.warn('Update failed', info);
    });
  },
});
