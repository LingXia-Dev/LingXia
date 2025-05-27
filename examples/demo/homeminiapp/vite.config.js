import { defineConfig } from 'vite';
import { LingXiaMiniAppBuilder } from '../../../lingxia-builder/vite-plugin.js';

export default defineConfig({
  plugins: [
    LingXiaMiniAppBuilder({
      minifyCode: process.env.NODE_ENV === 'production',
      removeComments: process.env.NODE_ENV === 'production',
      createPackage: process.env.NODE_ENV === 'production'
    })
  ],
  
  build: {
    rollupOptions: {
      input: 'app.js',
      external: () => false,
      output: {
        format: 'es',
        entryFileNames: 'temp.js'
      }
    }
  }
}); 