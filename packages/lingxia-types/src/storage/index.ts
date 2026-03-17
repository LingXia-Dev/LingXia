/**
 * Storage APIs
 * Corresponds to: lingxia-logic/src/storage.rs, env.rs
 */

export interface LxEnv {
  USER_DATA_PATH: string;
  USER_CACHE_PATH: string;
}

export interface Storage {
  get(key: string): unknown;
  set(key: string, value: unknown): void;
  remove(key: string): void;
  clear(): void;
  keys(): string[];
  has(key: string): boolean;
  size(): number;
}
