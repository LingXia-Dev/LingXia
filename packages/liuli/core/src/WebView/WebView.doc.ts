import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'WebView',
  category: 'media',
  status: 'preview',
  since: '2026-11-17',
  summary: 'An embedded web page, inline or expanded into a 70% sheet or full screen without reloading',
  description:
    'WebView frames an untrusted or packaged HTML document (`<webview>`) and optionally adds an expand control over its corner. ' +
    'Expanding opens a 70% bottom sheet or full-screen dialog holding the same `useWebView` session, so maps, previews and third-party widgets never reload or lose state when opened or dismissed.',
  keywords: ['webview', 'iframe', 'html', 'embed', 'map', 'picker', 'sheet', 'web'],
  propsType: 'WebViewProps',
  recipe: { map: 'webViewRecipe', parts: 'webViewParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The rounded inline frame that clips the embedded document.' },
    { part: 'webview', required: true, description: 'The inline `<webview>` surface.' },
    { part: 'expand', required: false, description: 'Media-docked corner button shown when `expand` is `sheet` or `full`.' },
    { part: 'panel', required: false, description: 'The 70% bottom sheet or full-screen `<dialog>` shown while expanded.' },
    { part: 'header', required: false, description: 'Header row in the expanded sheet or dialog with document title and close button.' },
    { part: 'stage', required: false, description: 'The expanded `<webview>` sharing the same `useWebView` session.' },
    { part: 'footer', required: false, description: 'Optional action bar below the expanded document, such as an address confirmation button.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Escape', effect: 'Closes the expanded sheet or full-screen dialog when `dismissOn` is `any` or `closeRequest`' },
    ],
    contrast: [
      { what: 'expand button', pair: ['on-media', 'media'], minimum: '4.5:1' },
      { what: 'sheet title', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
    ],
    notes: [
      'Both the inline `<webview>` and the expanded dialog are named by `label`; while expanded, the page behind is inert to assistive tech.',
    ],
  },
  guidance: {
    do: [
      'Use `expand="sheet"` for interactive cards like maps or previews where 70% of the screen gives enough room while keeping page context.',
      'Pass `actions` to place native confirmation controls below an expanded map or picker.',
      'Update only the `#hash` of `src` when passing coordinates or state into an already-loaded document without reloading it.',
    ],
    dont: [
      'Nest `WebView` inside a pressable control (`Button`, `Item` with `onClick`).',
      'Mount two separate `WebView` components without sharing a `useWebView` handle when moving the same document into a custom `BottomSheet` or `Dialog`.',
    ],
  },
  examples: [
    { name: 'ExpandableSheet', title: 'Inline card that expands into a 70% sheet' },
    { name: 'FullScreenWithActions', title: 'Full-screen expansion with a footer action' },
  ],
  gaps: [],
  related: ['BottomSheet', 'Dialog', 'VideoPlayer'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Embed a web page',
    order: 6,
    page: {
      lead: 'Use WebView to embed a web page or map inline and expand it into a sheet or full screen without reloading.',
      stage: 'bare',
      hero: 'ExpandableSheet',
      variants: [{ name: 'FullScreenWithActions', title: 'Full-screen expansion with a footer action' }],
    },
  },
};
