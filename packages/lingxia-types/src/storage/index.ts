/**
 * Key-value storage APIs.
 */

export interface Storage {
  get(key: string): unknown;
  set(key: string, value: unknown): void;
  remove(key: string): void;
  clear(): void;
  keys(): string[];
  has(key: string): boolean;
  size(): number;
}
