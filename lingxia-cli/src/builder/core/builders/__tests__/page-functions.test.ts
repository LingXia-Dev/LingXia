import { describe, it, expect } from 'vitest';
import { extractPageFunctionsFromSource } from '../page-functions.js';

const normalize = (input: string[]): string[] => input.slice().sort();

describe('extractPageFunctionsFromSource', () => {
  it('captures user-defined functions across syntaxes while skipping lifecycle hooks', () => {
    const source = `
      Page({
        data: {},
        onLoad() {},
        _internal() {},
        foo() {},
        bar: function () {},
        baz: async () => {},
        qux: value => value,
        nested: async function named () {}
      })
    `;

    expect(normalize(extractPageFunctionsFromSource(source))).toEqual(
      normalize(['foo', 'bar', 'baz', 'qux', 'nested'])
    );
  });

  it('ignores spread props and private helpers while keeping remaining entries', () => {
    const source = `
      const shared = {
        sharedFn() {}
      };

      Page({
        ...shared,
        keepMe(event) {
          console.log(event?.type);
        },
        async anotherOne () {},
        _skipMe: () => {},
        onShow() {},
        'stringKey': () => {},
        42: () => {}
      });
    `;

    expect(normalize(extractPageFunctionsFromSource(source))).toEqual(
      normalize(['keepMe', 'anotherOne', 'stringKey', '42'])
    );
  });

  it('handles TS assertions and nested parentheses around the Page config object', () => {
    const source = `
      type Foo = { submit(): void };

      Page(({
        submit () {}
      } as Foo));
    `;

    expect(extractPageFunctionsFromSource(source)).toEqual(['submit']);
  });

  it('returns empty array when no Page call is present', () => {
    const source = `
      const opts = { foo() {} };
      createPage(opts);
    `;

    expect(extractPageFunctionsFromSource(source)).toEqual([]);
  });
});
