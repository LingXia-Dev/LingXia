import type { FailOptions, FileOptions, TestBody, TestOptions } from "./types.js";

export interface TestApi {
  (title: string, body: TestBody): void;
  (title: string, options: TestOptions, body: TestBody): void;
  skip(title: string, body?: TestBody): void;
  skip(title: string, options: TestOptions, body?: TestBody): void;
  only(title: string, body: TestBody): void;
  only(title: string, options: TestOptions, body: TestBody): void;
  fixme(title: string, body?: TestBody): void;
  fixme(title: string, options: TestOptions, body?: TestBody): void;
  fail(title: string, body: TestBody): void;
  fail(title: string, options: FailOptions, body: TestBody): void;
  /** Restore product state before every attempt; required for retries. */
  reset(fn: TestBody): void;
  beforeEach(fn: TestBody): void;
  afterEach(fn: TestBody): void;
  /**
   * Defaults for every test in the calling file, such as `{ tags: ['routed'] }`.
   * File-scoped like the hooks; calling it again adds to the file's tags.
   */
  configure(options: FileOptions): void;
}
