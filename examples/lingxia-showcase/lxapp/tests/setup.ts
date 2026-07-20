export {};

const deadline = Date.now() + 30_000;
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
