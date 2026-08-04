import tailwindcss from '@tailwindcss/postcss';
import autoprefixer from 'autoprefixer';

export default {
  view: {
    cssConfig: async () => ({
      postcss: {
        // Tailwind 4 prefixes its own output; autoprefixer stays for the
        // hand-written page stylesheets that never pass through Tailwind.
        plugins: [tailwindcss(), autoprefixer()],
      },
    }),
  },
};
