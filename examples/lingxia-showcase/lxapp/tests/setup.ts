export {};

const deadline = Date.now() + 30_000;
let lastError: unknown = null;
let ready = false;

while (Date.now() < deadline) {
  try {
    const info = await lx.automation().lxapp().info();
    if (info.appid === 'lingxia-showcase') {
      ready = true;
      break;
    }
  } catch (error) {
    lastError = error;
  }
  await new Promise((resolve) => setTimeout(resolve, 100));
}

if (!ready) {
  throw new Error(`Showcase lxapp did not become current: ${String(lastError)}`);
}
