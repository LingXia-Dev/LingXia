import { lx, type AppLifecycleEventArgs } from '@lingxia/logic';

const moreFeedbackByLanguage = {
  "en-US": "Feedback",
  "zh-CN": "反馈",
} as const;

function moreFeedbackLabel(tag: string): string {
  const language = tag.toLowerCase().startsWith("zh") ? "zh-CN" : "en-US";
  return moreFeedbackByLanguage[language];
}

function applyMoreActions(tag = lx.app.language.get()) {
  lx.app.moreActions.replace([
    {
      icon: 'public/chat.png',
      label: moreFeedbackLabel(tag),
      onSelect: async () => {
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

/** What a reopen delivered, for the showcase desktop specs. */
const probe = { lastShow: undefined as AppLifecycleEventArgs | undefined };

App({
  onLaunch() {
    console.log('LingXia Showcase (WebView) launched');

    lx.app.language.watch((tag) => {
      applyMoreActions(tag);
    });

    lx.app.update.onReady(async () => {
      const applyNow = await lx.ui.confirm({
        title: 'Update Available',
        message: 'A new version is ready. Apply now?',
        cancelText: 'Later',
        confirmText: 'Apply',
      });
      if (applyNow) {
        lx.app.update.apply();
      }
    });
    lx.app.update.onFailed((info) => {
      console.warn('Update failed', info);
    });
  },

  onShow(options?: AppLifecycleEventArgs) {
    // Showcase desktop specs read what a reopen delivered.
    probe.lastShow = options;
    applyMoreActions();
  },

  // For `lxdev test`, which reads it through `appInstance().state`.
  state: probe,
});
