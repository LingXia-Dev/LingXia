import tailwindcss from 'tailwindcss';
import autoprefixer from 'autoprefixer';

export default {
  // staticDirs accepts glob patterns (e.g., pages/*/images) to copy matching directories.
  staticDirs: ['public', 'pages/*/images'],
  alias: {
    '@shared': 'shared'
  },
  sourceDirs: ['shared'],
  assetDir: 'assets',
  view: {
    cssConfig: async () => ({
      postcss: {
        plugins: [
          tailwindcss({
            config: './tailwind.config.js',
          }),
          autoprefixer(),
        ],
      },
    }),
  },
};
