/**
 * LxApp metadata APIs.
 */

export type LxAppReleaseType = 'release' | 'preview' | 'developer';

export interface LxAppInfo {
  appId: string;
  appName: string;
  version: string;
  releaseType: LxAppReleaseType;
}
