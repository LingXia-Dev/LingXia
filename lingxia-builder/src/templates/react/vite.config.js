import react from '@vitejs/plugin-react'

export default {
  plugins: [react()],
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
  esbuild: {
    jsx: 'automatic'
  },
  logLevel: 'warn'
}
