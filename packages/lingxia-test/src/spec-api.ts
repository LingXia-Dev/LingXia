import type { SpecBody, SpecOptions } from "./types.js";

export interface SpecApi {
  (title: string, body: SpecBody): void;
  (title: string, options: SpecOptions, body: SpecBody): void;
  skip(title: string, body?: SpecBody): void;
  skip(title: string, options: SpecOptions, body?: SpecBody): void;
  only(title: string, body: SpecBody): void;
  only(title: string, options: SpecOptions, body: SpecBody): void;
  fixme(title: string, body?: SpecBody): void;
  fixme(title: string, options: SpecOptions, body?: SpecBody): void;
  fail(title: string, body: SpecBody): void;
  fail(title: string, options: SpecOptions, body: SpecBody): void;
  beforeEach(fn: SpecBody): void;
}
