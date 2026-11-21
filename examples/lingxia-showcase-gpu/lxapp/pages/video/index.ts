import { definePage } from '@lingxia/types/page';
export default definePage({
  data: {},
  setup(page) {
    const actions = {
      // Logic drives the page's useVideo('hero') player by name.
      async pauseHero() {
        await page.video('hero').pause();
      },
      async playHero() {
        await page.video('hero').play();
      },
      async qualityFromLogic(params: { quality: string }) {
        await page.video('quality').setQuality(params.quality);
      },
    };
    return {
      actions,
    };
  },
});
