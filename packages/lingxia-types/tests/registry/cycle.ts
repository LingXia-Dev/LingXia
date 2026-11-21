import { definePage, type } from '../../src/page.js';

export const alpha = definePage({
  params: type<{ id: number }>(),
  setup() {
    return {
      actions: {
        async openBeta() { await lx.nav.go({ page: 'beta', params: { name: 'x' } }).opened; },
        async openBetaLoose(name: string) { await lx.nav.go({ page: 'beta', params: { name } }).opened; },
      },
    };
  },
});

export const beta = definePage({
  params: type<{ name: string }>(),
  setup() {
    return {
      actions: {
        async openAlpha() { await lx.nav.go({ page: 'alpha', params: { id: 1 } }).opened; },
        // @ts-expect-error alpha's id is a number
        async openAlphaWrong() { await lx.nav.go({ page: 'alpha', params: { id: '1' } }).opened; },
      },
    };
  },
});
