import { defineConfig } from '@lingxia/cli';

export default defineConfig({
  staticDirs: ['public'],
  alias: {
    '@shared': 'shared'
  },
  sourceDirs: ['shared'],
  assetDir: 'assets'
});
