import { packageText } from '@lingxia/view';
import words from './words';

/** Jinshu's word `key` in the page's language; an lxapp's catalogue overrides it by naming the key. */
export const jinshuText = (key: `@jinshu.${string}`, args?: Readonly<Record<string, unknown>>) => packageText(words, key, args);
