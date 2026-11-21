// Product Logic code: `lx` is the app's global.
export function userDataPath(): string {
  return lx.fs.userData;
}

export function hasStorageKey(key: string): Promise<boolean> {
  return lx.storage().has(key);
}
