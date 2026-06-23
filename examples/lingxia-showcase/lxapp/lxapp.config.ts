import tailwindcss from 'tailwindcss';
import autoprefixer from 'autoprefixer';

export default {
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
