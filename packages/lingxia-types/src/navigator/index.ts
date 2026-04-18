/**
 * LxApp navigator APIs.
 */

import type { PageQuery } from '../ui';

export type LxAppEnvVersion = 'release' | 'preview' | 'develop';

export interface NavigateToLxAppOptions {
  appId: string;
  page?: string;
  path?: string;
  query?: PageQuery;
  envVersion?: LxAppEnvVersion;
  targetVersion?: string;
}
