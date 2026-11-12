// Renders Liuli examples in Node through `@lingxia/view/testing`. Each render builds a throwaway
// lxapp that links this package into its node_modules, so Liuli compiles as an installed
// dependency would: its token file applies and its files are named by package.
import { mkdtemp, mkdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from '@lingxia/view/testing';

const core = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

// A phone-sized environment: the testing entry supplies none, and `vh` classes need a viewport.
const phone = { viewport: { width: 390, height: 844 }, pointer: 'coarse', direction: 'ltr', theme: 'light' };

/**
 * Builds and renders a one-page lxapp. `page` is its source, or a function from the linked
 * package's directory to the source; it may import `@liulidesign/core/…`.
 */
export async function renderPage(page, view = {}) {
  const app = await mkdtemp(join(tmpdir(), 'liuli-unit-'));
  try {
    const linked = join(app, 'node_modules/@liulidesign/core');
    await writeFile(join(app, 'lxapp.json'), '{}');
    await mkdir(dirname(linked), { recursive: true });
    await symlink(core, linked, 'dir');
    await writeFile(join(app, 'page.tsx'), typeof page === 'function' ? page(linked) : page);
    return await render(join(app, 'page.tsx'), { ...view, env: { ...phone, ...view.env } }, { root: app });
  } finally {
    await rm(app, { recursive: true, force: true });
  }
}

/** Renders `<Example />` from `src/<module>.examples.tsx` inside a page. */
export function renderExample(module, example, view = {}) {
  // Examples are not exported, so the page imports the file by its path through the link.
  return renderPage(
    linked => `import { ${example} as Example } from ${JSON.stringify(join(linked, 'src', `${module}.examples`))};\n` +
      'export function render() { return <page><Example /></page>; }\n',
    view,
  );
}
