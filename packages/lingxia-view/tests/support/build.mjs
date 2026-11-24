// `buildPages` with the paths the CLI always names: no catalogues unless the test gives some, and
// the inputs record in the project.
import { join } from 'node:path';
import { buildPages } from '../../compiler/index.mjs';

export const build = (root, pages, output, catalogs = {}, options = {}) =>
  buildPages(root, pages, output, catalogs, { messages: null, inputsRecord: join(root, '.lingxia', 'gpu-inputs.json'), ...options });
