App({
  onLaunch() {
    console.log('LingXia Chat launched');

    lx.setMoreActions([
      {
        icon: 'public/chat.png',
        label: 'Feedback',
        onClick: async () => {
          try {
            await lx.openSurface({
              page: 'feedback',
              as: 'float',
              position: 'bottom',
              size: { width: '100%', height: '80%' },
              interaction: {
                closeButton: true,
                dismiss: 'manual',
                modal: true,
              },
            });
          } catch (error) {
            console.warn('failed to open feedback surface', error);
          }
        },
      },
    ]);

    const um = lx.getUpdateManager();
    um.onUpdateReady(async (info) => {
      if (info?.isForceUpdate) {
        console.log('Force update ready; apply immediately');
        um.applyUpdate();
        return;
      }

      const applyNow = await lx.showModal({
        title: 'Update Available',
        content: 'A new version is ready. Apply now?',
        showCancel: true,
        cancelText: 'Later',
        confirmText: 'Apply',
      });
      if (!applyNow.canceled) {
        um.applyUpdate();
      }
    });
    um.onUpdateFailed((info) => {
      console.warn('Update failed', info);
    });
  },
});
