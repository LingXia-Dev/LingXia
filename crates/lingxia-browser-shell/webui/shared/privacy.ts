export type TimeRange = 'lastHour' | 'last24Hours' | 'last7Days' | 'last4Weeks' | 'allTime';

export const TIME_RANGES: readonly TimeRange[] = ['lastHour', 'last24Hours', 'last7Days', 'last4Weeks', 'allTime'];

export interface ClearBrowsingDataInput {
  timeRange: TimeRange;
  history: boolean;
  cache: boolean;
  siteData: boolean;
}

export interface ClearBrowsingDataResult {
  historyRemoved: number;
}
