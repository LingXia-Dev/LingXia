/**
 * LxApp navigator APIs
 * Corresponds to: lingxia-logic/src/navigator.rs
 */

export interface NavigateToLxAppOptions {
  appId: string;
  path?: string;
  envVersion?: 'develop' | 'trial' | 'release';
}
