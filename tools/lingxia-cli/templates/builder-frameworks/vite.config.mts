import fs from 'node:fs';
import path from 'node:path';
import { defineConfig } from 'vite';
__PLUGIN_IMPORT__

const projectRoot = __PROJECT_ROOT_JSON__;
const buildDir = __BUILD_DIR_JSON__;
const inputEntries = __INPUT_ENTRIES_JSON__;

const resolveWorkspaceSourceEntry = (packageName, sourceEntry) => {
  const packageDir = path.resolve(projectRoot, 'node_modules', ...packageName.split('/'));
  const entryPath = path.join(packageDir, sourceEntry);
  return fs.existsSync(entryPath) ? entryPath : null;
};

const normalizeModuleId = (id) => id.split('?')[0].replaceAll('\\', '/');

const hasModuleBasename = (id, packageMarker, moduleNames) =>
  id.includes(packageMarker) && moduleNames.some((moduleName) => id.includes(`/${moduleName}.`));

const hasLingxiaPackageModule = (id, packageName, workspaceDir, moduleNames) =>
  hasModuleBasename(id, `/${packageName}/`, moduleNames) ||
  hasModuleBasename(id, `/packages/${workspaceDir}/`, moduleNames);

const manualChunks = (rawId) => {
  const id = normalizeModuleId(rawId);
  if (id.includes('__page_bridge_runtime__.js')) return 'page-bridge-runtime';
  if (id.includes('/node_modules/react/') || id.includes('/node_modules/react-dom/') || id.includes('/node_modules/scheduler/')) return 'react-runtime';
  if (id.includes('/node_modules/vue/') || id.includes('/node_modules/@vue/')) return 'vue-runtime';
  if (
    hasLingxiaPackageModule(id, '@lingxia/react', 'lingxia-react', ['hook']) ||
    hasLingxiaPackageModule(id, '@lingxia/vue', 'lingxia-vue', ['hook'])
  ) return 'lingxia-page-runtime';
  if (
    hasLingxiaPackageModule(id, '@lingxia/react', 'lingxia-react', ['LxVideo']) ||
    hasLingxiaPackageModule(id, '@lingxia/vue', 'lingxia-vue', ['LxVideo']) ||
    hasLingxiaPackageModule(id, '@lingxia/elements', 'lingxia-elements', ['video'])
  ) return 'lingxia-video-runtime';
  if (
    hasLingxiaPackageModule(id, '@lingxia/react', 'lingxia-react', ['LxPicker']) ||
    hasLingxiaPackageModule(id, '@lingxia/vue', 'lingxia-vue', ['LxPicker']) ||
    hasLingxiaPackageModule(id, '@lingxia/elements', 'lingxia-elements', ['picker', 'picker-web'])
  ) return 'lingxia-picker-runtime';
  if (
    hasLingxiaPackageModule(id, '@lingxia/react', 'lingxia-react', ['LxNavigator']) ||
    hasLingxiaPackageModule(id, '@lingxia/vue', 'lingxia-vue', ['LxNavigator']) ||
    hasLingxiaPackageModule(id, '@lingxia/elements', 'lingxia-elements', ['navigator'])
  ) return 'lingxia-navigator-runtime';
  if (
    hasLingxiaPackageModule(id, '@lingxia/react', 'lingxia-react', ['LxInput', 'LxTextarea', 'text_component_shared']) ||
    hasLingxiaPackageModule(id, '@lingxia/vue', 'lingxia-vue', ['LxInput', 'LxTextarea', 'text_component_shared']) ||
    hasLingxiaPackageModule(id, '@lingxia/elements', 'lingxia-elements', ['input', 'textarea', 'text_component_shared', 'text_component_native_attrs'])
  ) return 'lingxia-text-runtime';
  if (
    hasLingxiaPackageModule(id, '@lingxia/react', 'lingxia-react', ['index']) ||
    hasLingxiaPackageModule(id, '@lingxia/vue', 'lingxia-vue', ['index', 'types']) ||
    hasLingxiaPackageModule(id, '@lingxia/elements', 'lingxia-elements', ['index', 'nativecomponent', 'component', 'dom', 'platform', 'types', 'native_component_wrapper_shared'])
  ) return 'lingxia-runtime';
  return undefined;
};

__MAYBE_CONFIG_IMPORT__

const viewConfig = projectConfig.view ?? {};
const css = typeof viewConfig.cssConfig === 'function' ? await viewConfig.cssConfig(buildDir) : undefined;

const workspaceAliases = [
  ['@lingxia/react', resolveWorkspaceSourceEntry('@lingxia/react', 'src/index.ts')],
  ['@lingxia/vue', resolveWorkspaceSourceEntry('@lingxia/vue', 'src/index.ts')],
]
  .filter(([, replacement]) => typeof replacement === 'string')
  .map(([find, replacement]) => ({ find, replacement }));

const alias = [
  { find: /^@\//, replacement: `${projectRoot}/` },
  { find: /^\/public\//, replacement: `${path.resolve(projectRoot, 'public')}/` },
  ...workspaceAliases,
  ...Object.entries(projectConfig.alias ?? {})
    .map(([find, replacement]) => {
      if (typeof replacement !== 'string') return null;
      if (find === '@') {
        const normalized = replacement.endsWith('/') ? replacement : `${replacement}/`;
        return { find: /^@\//, replacement: path.resolve(projectRoot, normalized) };
      }
      return { find, replacement: path.resolve(projectRoot, replacement) };
    })
    .filter(Boolean),
];

export default defineConfig({
  root: buildDir,
  base: '/',
  logLevel: 'warn',
  plugins: frameworkPlugins,
  css,
  resolve: { alias, dedupe: ['react', 'react-dom', 'vue'] },
  build: {
    target: 'esnext',
    outDir: path.join(buildDir, 'dist'),
    emptyOutDir: true,
    sourcemap: __SOURCEMAP__,
    minify: __MINIFY__,
    cssMinify: __CSS_MINIFY__,
    rollupOptions: {
      input: inputEntries,
      output: {
        entryFileNames: 'pages/[name]/[name].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]',
        manualChunks,
      },
    },
  },
});
