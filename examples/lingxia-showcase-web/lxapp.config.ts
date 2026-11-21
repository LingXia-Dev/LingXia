import tailwindcss from '@tailwindcss/postcss';
import autoprefixer from 'autoprefixer';

export default {
  view: {
    cssConfig: async () => ({
      postcss: {
        plugins: [tailwindcss(), autoprefixer()],
      },
    }),
  },
};
