// Renders Liuli examples in Node through `@lingxia/view/testing`. Each render builds a throwaway
// lxapp that links the package into its node_modules, so Liuli compiles as an installed
// dependency would: its token file applies and its files are named by package. Another Liuli
// package imports `harnessFor` from `@liulidesign/core/testing`.
import { readFileSync } from 'node:fs';
import { mkdtemp, mkdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render, textOf } from '@lingxia/view/testing';

const core = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const nameOf = dir => JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')).name;

/**
 * The render helpers for one Liuli package (its folder). Its throwaway apps link that package and
 * core, since every Liuli package builds on core.
 */
export function harnessFor(packageDir) {
  const own = resolve(packageDir);
  const links = [...new Set([own, core])].map(dir => ({ dir, name: nameOf(dir) }));

  /**
   * Builds and renders a one-page lxapp. `page` is its source, or a function from the linked
   * package's directory to the source; it may import any linked package.
   */
  async function renderPage(page, view = {}) {
    const app = await mkdtemp(join(tmpdir(), 'liuli-unit-'));
    try {
      await writeFile(join(app, 'lxapp.json'), '{}');
      for (const { dir, name } of links) {
        const linked = join(app, 'node_modules', name);
        await mkdir(dirname(linked), { recursive: true });
        await symlink(dir, linked, 'junction');
      }
      const linked = join(app, 'node_modules', links[0].name);
      await writeFile(join(app, 'page.tsx'), typeof page === 'function' ? page(linked) : page);
      // Theme steps read as the built-in pack's points, the numbers these tests state.
      return await render(join(app, 'page.tsx'), view, { root: app, points: true });
    } finally {
      await rm(app, { recursive: true, force: true });
    }
  }

  /** Renders `<Example />` from `src/<module>.examples.tsx` inside a page. */
  function renderExample(module, example, view = {}) {
    // Examples are not exported, so the page imports the file by its path through the link.
    return renderPage(
      linked => `import { ${example} as Example } from ${JSON.stringify(join(linked, 'src', `${module}.examples`))};\n` +
        'export function render() { return <page><Example /></page>; }\n',
      view,
    );
  }

  return { renderPage, renderExample, described };
}

export const { renderPage, renderExample } = harnessFor(core);

// As the device reads it: a hidden target is still read, but no `aria-hidden` subtree under it.
const readable = node => node.children?.length ? node.children.map(child => child.props.accessibilityHidden === 1 ? '' : readable(child)).join('') : textOf(node);

/** The text of each element `node` names in `aria-describedby`, in order, without the `aria-hidden` text under them; the device reads these after the name. */
export function described(screen, node) {
  const ids = node.props.ariaDescribedBy?.split(' ') ?? [];
  return ids.map(id => {
    const target = screen.queryAll(other => other.props.elementId === id);
    return target.length === 1 ? readable(target[0]) : `<${target.length} elements with id ${id}>`;
  });
}

/** The spinner mark: the box that turns, whatever draws inside it. */
export const isSpinner = node => /"name":"spin"/.test(node.props.animation ?? '');
