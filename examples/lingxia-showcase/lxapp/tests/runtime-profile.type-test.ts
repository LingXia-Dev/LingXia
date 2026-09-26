import { rawAutomation } from '@lingxia/test';

// The automation root is an import; `lx` in a spec program means only the app's.
void rawAutomation().lxapp().network;

// @ts-expect-error The automation test runtime is not a page WebView.
void document;
// @ts-expect-error The automation test runtime does not expose Node globals.
void process;
