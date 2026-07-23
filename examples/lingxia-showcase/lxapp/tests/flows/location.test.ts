import { test } from '@rongjs/test';
import type {
  DesktopAxNode,
  DesktopDriver,
  DesktopWindowInfo,
  LxAppDriver,
} from 'lingxia-types';

async function platform(app: LxAppDriver): Promise<string> {
  const args = test.args as Record<string, string>;
  if (args.platform) return args.platform.toLocaleLowerCase();
  return app.eval({
    script: 'return String(lx.app.getBaseInfo().os || "").toLowerCase()',
  }) as Promise<string>;
}

function locationPrompt(windows: DesktopWindowInfo[]): DesktopWindowInfo | undefined {
  return windows.find((window) => (
    window.visible && window.process.toLocaleLowerCase() === 'corelocationagent'
  ));
}

function allowButton(nodes: DesktopAxNode[]): DesktopAxNode | undefined {
  return nodes.find((node) => {
    const name = node.name.trim().toLocaleLowerCase();
    return name.startsWith('allow')
      || name === 'ok'
      || (name.includes('允许') && !name.startsWith('不'));
  });
}

async function waitForPromptToClose(desktop: DesktopDriver, timeoutMs = 5_000): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!locationPrompt(await desktop.windows())) return true;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}

async function allowLocation(
  desktop: DesktopDriver,
  prompt: DesktopWindowInfo,
  permissions: { accessibility: boolean; input: boolean },
): Promise<void> {
  if (!permissions.accessibility) {
    throw new Error('macOS location permission requires Accessibility access for automation');
  }

  const buttons = await desktop.ax.query({
    window: prompt.id,
    match: 'role:button',
    all: true,
  });
  const allow = allowButton(buttons);
  if (!allow) throw new Error('Could not find Allow in the macOS location permission dialog');

  console.info('Accepting the macOS location permission dialog with AXPress');
  await desktop.ax.invoke({
    window: prompt.id,
    match: `id:${allow.id}`,
  });
  if (await waitForPromptToClose(desktop)) {
    console.info('The macOS location permission dialog closed after AXPress');
    return;
  }

  if (!permissions.input) {
    throw new Error('AXPress did not close the macOS location permission dialog');
  }

  await desktop.window.focus({ window: prompt.id });
  console.info('AXPress did not close the dialog; retrying with a foreground pointer click');
  await desktop.pointer.click({
    at: [
      allow.rect.x + Math.floor(allow.rect.w / 2),
      allow.rect.y + Math.floor(allow.rect.h / 2),
    ],
  });
  if (!await waitForPromptToClose(desktop)) {
    throw new Error('Allow was invoked, but the macOS location permission dialog stayed open');
  }
  console.info('The macOS location permission dialog closed after the pointer click');
}

const targetPlatform = (test.args as Record<string, string>).platform?.toLocaleLowerCase();
const locationTest = targetPlatform && targetPlatform !== 'macos' ? test.skip : test;

locationTest('handles the macOS location permission sheet when it appears', async () => {
  const auto = lx.automation();
  const app = auto.lxapp();
  if (await platform(app) !== 'macos') return;
  const doctor = await auto.desktop.doctor();
  const { permissions } = doctor;

  await app.nav.relaunch({ page: 'location' });
  await app.page.waitFor({ page: 'location', css: 'button', state: 'visible' });
  await app.page.click({ page: 'location', css: 'button', index: 0 });

  const deadline = Date.now() + 15_000;
  let requestStarted = false;
  let promptHandled = false;
  while (Date.now() < deadline) {
    const prompt = locationPrompt(await auto.desktop.windows());
    if (prompt) {
      console.info(`Detected the macOS location permission dialog (${prompt.id})`);
      if (doctor.capabilities.screenshot && permissions.screen_recording) {
        const screenshot = await auto.desktop.screenshot({ window: prompt.id });
        await test.attach?.('macos-location-permission.png', {
          mimeType: 'image/png',
          base64: screenshot.base64,
        });
      }
      await allowLocation(auto.desktop, prompt, permissions);
      promptHandled = true;
    }

    const state = await app.eval({
      script: `
        const page = getCurrentPages().find((candidate) => candidate.route.includes('/location/'));
        return {
          loading: !!page?.data?.isLoading,
          location: page?.data?.location ?? null,
          error: String(page?.data?.locationError ?? ''),
        };
      `,
    }) as { loading: boolean; location: unknown; error: string };
    requestStarted ||= state.loading;
    if (!state.loading && (state.location !== null || state.error.length > 0)) {
      if (locationPrompt(await auto.desktop.windows())) {
        throw new Error('Location request settled while its macOS permission dialog stayed open');
      }
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(
    requestStarted
      ? promptHandled
        ? 'Location request did not settle after the macOS permission dialog was accepted'
        : 'Location request did not settle; its macOS permission dialog did not appear'
      : 'Location request was not started by the page click',
  );
});
