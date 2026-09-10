const moreFeedbackByLanguage = {
  "en-US": "Feedback",
  "zh-CN": "反馈",
} as const;

function moreFeedbackLabel(tag: string): string {
  const language = tag.toLowerCase().startsWith("zh") ? "zh-CN" : "en-US";
  return moreFeedbackByLanguage[language];
}

function applyMoreActions(tag = lx.app.displayLanguage.get()) {
  lx.setMoreActions([
    {
      icon: 'public/chat.png',
      label: moreFeedbackLabel(tag),
      onClick: async () => {
        try {
          await lx.surface.openPage('feedback', {
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
}

App({
  onLaunch() {
    console.log('LingXia Chat launched');

    lx.app.displayLanguage.watch((tag) => {
      applyMoreActions(tag);
    });
    lx.app.control?.displayLanguage.watchPreference(() => {
      applyMoreActions();
    });

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

  onShow() {
    applyMoreActions();
  },
});
