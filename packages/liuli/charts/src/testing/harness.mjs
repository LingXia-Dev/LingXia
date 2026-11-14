// The render helpers of Liuli core's test harness, bound to this package.
import { fileURLToPath } from 'node:url';
import { harnessFor } from '@liulidesign/core/testing';

export const { renderPage, renderExample, described } = harnessFor(fileURLToPath(new URL('../..', import.meta.url)));
