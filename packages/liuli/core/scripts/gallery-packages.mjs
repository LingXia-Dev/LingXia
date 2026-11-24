// The Liuli packages the gallery shows, named in gallery/packages.json: a folder beside them is
// never read, whatever it holds.
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

/** The listed packages other than `folder`, each with its catalog.json path; a listed one without a catalog throws. */
export function galleryPackages(root, folder) {
  const listed = JSON.parse(readFileSync(join(root, 'gallery', 'packages.json'), 'utf8'));
  return listed.filter(name => name !== folder).sort().map(name => {
    const catalog = join(root, name, 'catalog.json');
    if (!existsSync(catalog)) throw Error(`gallery/packages.json lists ${name}, which has no catalog.json`);
    return { name, catalog };
  });
}
