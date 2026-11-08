Page({
  data: {},
  // Logic drives the page's useVideo('hero') player by name.
  async pauseHero() {
    await lx.createVideoContext('hero').pause();
  },
  async playHero() {
    await lx.createVideoContext('hero').play();
  },
  async qualityFromLogic(params: { quality: string }) {
    await lx.createVideoContext('quality').setQuality(params.quality);
  },
});
