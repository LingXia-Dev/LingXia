declare global {
  interface Window {
    native?: Record<string, unknown>;
  }
}

console.log('Rust native APIs are available on window.native', window.native);

export {};
