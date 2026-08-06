void lx.automation();

// @ts-expect-error The automation test runtime does not expose app Logic APIs.
void lx.app;
// @ts-expect-error The automation test runtime does not expose lxapp storage.
void lx.getStorage;
// @ts-expect-error The automation test runtime is not a page WebView.
void document;
// @ts-expect-error The automation test runtime does not expose Node globals.
void process;
