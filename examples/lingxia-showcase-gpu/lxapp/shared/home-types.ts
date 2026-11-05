import type { DisplayLanguagePreference } from './display-language';

export type AppearancePreference = 'auto' | 'light' | 'dark';
export interface HomePageData {
  greeting: string;
  imageUrl: string;
  ipAddr: string;
  greetCount: number;
  appVersion: string;
  appearance: { preference: AppearancePreference; resolved: 'light' | 'dark' };
  displayLanguage: { preference: DisplayLanguagePreference; resolved: string };
}
export interface GreetingRequest { name: string }
export interface AppearanceRequest { preference: AppearancePreference }
export interface DisplayLanguageRequest { preference: DisplayLanguagePreference }
export interface HomeActions {
  greet(request: GreetingRequest): Promise<void>;
  setAppearance(request: AppearanceRequest): Promise<void>;
  setDisplayLanguage(request: DisplayLanguageRequest): Promise<void>;
}
