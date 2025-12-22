import typescript from '@rollup/plugin-typescript';
import terser from '@rollup/plugin-terser';

export default {
  input: 'src/index.ts',
  output: {
    file: 'dist/runtime.js',
    format: 'iife',
    name: 'LingXiaRuntime',
    sourcemap: false,
  },
  plugins: [
    typescript({
      tsconfig: './tsconfig.json',
      declaration: false,
      declarationDir: undefined,
    }),
    terser({
      format: {
        comments: false,
      },
    }),
  ],
};
