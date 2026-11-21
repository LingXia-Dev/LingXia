import { definePage } from '@lingxia/types/page';
export default definePage({
  data: { packagedMedia: false },
  setup(page) {
    return {
      onLoad(options: { media?: string } = {}) {
        if (options.media === 'packaged') page.update(d => { d.packagedMedia = true; });
      },
    };
  },
});
