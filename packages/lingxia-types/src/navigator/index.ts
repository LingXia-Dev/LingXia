/**
 * LxApp navigator APIs.
 */

export interface NavigateToLxAppOptions {
  appId: string;
  path?: string;
  envVersion?: 'develop' | 'trial' | 'release';
}
