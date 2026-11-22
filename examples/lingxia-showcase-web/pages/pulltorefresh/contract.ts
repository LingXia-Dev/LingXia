import type { PageShape } from '../../shared/lib/page-shape';

export type PullToRefreshPage = PageShape<
  { refreshCount: number; lastRefreshTime: string | null; isRefreshing: boolean },
  { startRefresh(): Promise<void>; stopRefresh(): Promise<void> }
>;
