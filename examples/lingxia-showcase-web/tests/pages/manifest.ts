// Every page in lxapp.json order, a text it renders, and its baked document title.
export const PAGE_EXPECTATIONS = [
  { page: 'home', title: 'WebView Demos', text: 'What a WebView page can show' },
  { page: 'chat', title: 'AI Chat', text: 'AI Chat' },
  { page: 'feedback', title: 'Feedback', text: 'Help us improve' },
  { page: 'about', title: 'About', text: 'LingXia Showcase (WebView)' },
  { page: 'picker', title: 'LxPicker', text: 'Component like input, tap to show picker' },
  { page: 'swiper', title: 'LxMediaSwiper', text: 'Media Swiper' },
  { page: 'navigator', title: 'Navigator', text: 'Declarative navigation component' },
  { page: 'video', title: 'Video', text: 'HTML Video' },
  { page: 'pullToRefresh', title: 'Pull Down Refresh', text: 'Refresh Status' },
  { page: 'channel', title: 'Channel', text: 'Recent Ticks' },
  { page: 'bridge-repro', title: 'Bridge Repro', text: 'Restart the lxapp to repeat.' },
  { page: 'spec-embedded', title: 'Embedded Document', text: 'Host page of the embedded-document specs.' },
] as const;

export const PAGES = PAGE_EXPECTATIONS.map(({ page }) => page);
