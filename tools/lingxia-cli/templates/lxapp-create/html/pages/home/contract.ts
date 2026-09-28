import type { PageContract } from '@lingxia/types/page';

export type HomePage = PageContract<
  { greeting: string; greetCount: number },
  { greet(payload: { name: string }): Promise<void> }
>;
