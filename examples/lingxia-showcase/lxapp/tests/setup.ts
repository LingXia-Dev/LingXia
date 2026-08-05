export {};

// The first WebView creation can stall well past 30s on a loaded shared CI
// runner (run 30960425260 hung ~40s in WebView2 startup on Windows); a host
// that never becomes ready still fails at the deadline.
const deadline = Date.now() + 90_000;
let lastError: unknown = null;
let ready = false;

while (Date.now() < deadline) {
  try {
    const app = lx.automation().lxapp();
    const info = await app.info();
    const logicReady = info.appid === 'lingxia-showcase'
      && (await app.eval({ script: 'true', timeoutMs: 20_000 })) === true;
    if (logicReady) {
      ready = true;
      break;
    }
  } catch (error) {
    lastError = error;
  }
  await new Promise((resolve) => setTimeout(resolve, 100));
}

if (!ready) {
  throw new Error(`Showcase lxapp and Logic runtime did not become ready: ${String(lastError)}`);
}
