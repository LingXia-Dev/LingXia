// Product Logic code: `lx` is imported, as every Logic module does.
import { lx } from '@lingxia/logic';

export function userDataPath(): string {
  return lx.fs.userData;
}

export function hasStorageKey(key: string): Promise<boolean> {
  return lx.storage.has(key);
}
