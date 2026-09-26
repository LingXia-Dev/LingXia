// Product Logic code: `lx` is the app's global.
export function userDataPath(): string {
  return lx.env.USER_DATA_PATH;
}

export function hasStorageKey(key: string): Promise<boolean> {
  return lx.getStorage().has(key);
}
