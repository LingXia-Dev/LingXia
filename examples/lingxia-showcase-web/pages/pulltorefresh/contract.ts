import type { PageContract } from '@lingxia/types/page';

export type PullToRefreshPage = PageContract<
  { refreshCount: number; lastRefreshTime: string | null; isRefreshing: boolean },
  { startRefresh(): Promise<void>; stopRefresh(): Promise<void> }
>;
