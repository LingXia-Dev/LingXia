import vue from '@vitejs/plugin-vue'

export default {
  plugins: [vue()],
  build: {
    rollupOptions: {
      output: {
        entryFileNames: 'main.js',
        chunkFileNames: 'chunks/[name]-[hash].js',
        assetFileNames: 'assets/[name].[ext]'
      }
    },
    outDir: 'dist',
    emptyOutDir: true,
    target: 'es2015',
    minify: false
  },
  logLevel: 'warn'
}