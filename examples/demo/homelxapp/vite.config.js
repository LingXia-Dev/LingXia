import { defineConfig } from 'vite';
import { lingxiaPlugin } from '../../../lingxia-builder/vite-plugin.js';

export default defineConfig({
  plugins: [
    lingxiaPlugin({
      appConfig: './app.json',
      outputDir: 'dist',
      createPackage: process.env.NODE_ENV === 'production',
      minifyCode: process.env.NODE_ENV === 'production'
    })
  ],

  build: {
    rollupOptions: {
      input: 'app.js',
      external: () => false,
      output: {
        format: 'es',
        entryFileNames: '[name].js',
        dir: '.lingxia-build' 
      }
    }
  }
});