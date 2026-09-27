// An app declares its own error codes once; specs then expect them by code.
import { spec } from '@lingxia/test';

declare module '@lingxia/test' {
  interface AppErrorCodes {
    E_QUOTA: true;
    E_PAYMENT_DECLINED: true;
  }
}

spec('app-defined codes are expected by code', async (t) => {
  const quota = () => Promise.reject(Object.assign(new Error('over quota'), { code: 'E_QUOTA' }));
  await t.reject(quota, { code: 'E_QUOTA' });
  // LingXia's own codes stay accepted next to the app's.
  await t.reject(quota, { code: 'E_TIMEOUT' });
  // @ts-expect-error A code the app did not declare does not compile.
  await t.reject(quota, { code: 'E_QUOTA_EXCEEDED' });
});

spec.fail('declined payment', { expected: { code: 'E_PAYMENT_DECLINED' } }, async () => {});
// @ts-expect-error `expected.code` is checked the same way.
spec.fail('undeclared', { expected: { code: 'E_CARD_EXPIRED' } }, async () => {});
