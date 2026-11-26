import assert from 'node:assert/strict';
import { test } from 'node:test';
import { LxError, type TerminalColorSchemeDetails, type TerminalSettingsPatch, type TerminalSettingsSnapshot } from '@lingxia/logic';
import { createLxMock, mount } from '@lingxia/logic/testing';
import settingsPage from '../pages/settings/index.ts';
import { PACKAGED, mergeSchemes, toneOf } from '../pages/settings/schemes.ts';

const light: TerminalColorSchemeDetails = {
  name: 'lingxia-light',
  source: 'built_in',
  scheme: { ...PACKAGED[0].scheme, name: 'LingXia Light', background: '#FAFAF7', foreground: '#1A1A1A' },
};
const dark: TerminalColorSchemeDetails = {
  name: 'lingxia-dark',
  source: 'built_in',
  scheme: { ...PACKAGED[0].scheme, name: 'LingXia Dark', background: '#15181D', foreground: '#E6E6E6' },
};

/** A host terminal that keeps its settings in memory and checks revisions as the real one does. */
function fakeTerminal(options: { appearance?: 'light' | 'dark'; conflicts?: number } = {}) {
  let conflicts = options.conflicts ?? 0;
  const calls: { update: TerminalSettingsPatch[]; import: string[] } = { update: [], import: [] };
  const schemes = [light, dark];
  const defaults = {
    font: { family: ['JetBrains Mono'], size: 13, lineHeight: 1, ligatures: true },
    theme: { light: 'lingxia-light', dark: 'lingxia-dark' },
  };
  let snapshot: TerminalSettingsSnapshot = {
    revision: 1,
    defaults,
    overrides: {},
    value: structuredClone(defaults),
    effective: {
      appearance: options.appearance ?? 'dark',
      colorScheme: options.appearance === 'light' ? 'lingxia-light' : 'lingxia-dark',
      font: { family: 'JetBrains Mono', missing: [], fellBack: false },
    },
    warnings: [],
  };
  const outdated = () => new LxError('revision moved', { code: 'E_INVALID_STATE', reason: 'revisionOutdated' });
  const check = (ifRevision: number) => {
    if (conflicts > 0) {
      conflicts -= 1;
      snapshot = { ...snapshot, revision: snapshot.revision + 1 };
      throw outdated();
    }
    if (ifRevision !== snapshot.revision) throw outdated();
  };
  const terminal = {
    settings: {
      get: async () => structuredClone(snapshot),
      async update({ ifRevision, ...patch }: TerminalSettingsPatch & { ifRevision: number }) {
        check(ifRevision);
        calls.update.push(patch);
        const value = {
          font: { ...snapshot.value.font, ...patch.font },
          theme: { ...snapshot.value.theme, ...patch.theme },
        };
        snapshot = { ...snapshot, revision: snapshot.revision + 1, value };
        return structuredClone(snapshot);
      },
      async reset({ ifRevision }: { ifRevision: number }) {
        check(ifRevision);
        snapshot = { ...snapshot, revision: snapshot.revision + 1, value: structuredClone(defaults), overrides: {} };
        return structuredClone(snapshot);
      },
      onChange: () => () => {},
    },
    colorSchemes: {
      list: async () => structuredClone(schemes),
      async import({ text, name }: { text: string; name?: string }) {
        calls.import.push(name ?? '');
        const details: TerminalColorSchemeDetails = { name: name ?? 'imported', source: 'imported', scheme: JSON.parse(text) };
        schemes.push(details);
        return details;
      },
      async importFile() {
        const details: TerminalColorSchemeDetails = {
          name: 'gruvbox', source: 'imported', scheme: { ...dark.scheme, name: 'Gruvbox' },
        };
        schemes.push(details);
        return { status: 'ok' as const, scheme: details };
      },
      createPreview: () => { throw new Error('the page applies at once and never previews'); },
    },
    fonts: {
      list: async () => [
        { family: 'Menlo', monospace: true, ligatures: false, nerdIcons: false },
        { family: 'Helvetica', monospace: false, ligatures: false, nerdIcons: false },
        { family: 'JetBrains Mono', monospace: true, ligatures: true, nerdIcons: false },
      ],
    },
  };
  const lx = createLxMock({
    supports: (feature: string) => feature === 'host.terminal',
    host: { terminal } as never,
  });
  return { lx, calls, snapshot: () => snapshot };
}

function open(host: ReturnType<typeof fakeTerminal>) {
  return mount(settingsPage, { lx: host.lx });
}

test('loads the snapshot, every scheme and only the monospaced fonts, sorted', async () => {
  const host = fakeTerminal();
  const page = await open(host);
  try {
    assert.equal(page.data.status, 'ready');
    assert.equal(page.data.snapshot?.revision, 1);
    assert.deepEqual(page.data.fonts, ['JetBrains Mono', 'Menlo']);
    assert.deepEqual(page.data.schemes.map((s) => s.name).sort(), ['dracula', 'lingxia-dark', 'lingxia-light', 'nord', 'solarized-dark']);
    assert.equal(page.data.inlineImages, null);
  } finally {
    await page.unmount();
  }
});

test('a host without the terminal shows the page as unavailable', async () => {
  const page = await mount(settingsPage, { lx: createLxMock({ supports: () => false }) });
  try {
    assert.equal(page.data.status, 'unavailable');
  } finally {
    await page.unmount();
  }
});

test('a font change applies at once as a field-level patch', async () => {
  const host = fakeTerminal();
  const page = await open(host);
  try {
    await page.actions.setFont({ size: 15 });
    await page.actions.setFont({ family: 'Menlo' });
    assert.deepEqual(host.calls.update, [{ font: { size: 15 } }, { font: { family: ['Menlo'] } }]);
    assert.equal(page.data.snapshot?.value.font.size, 15);
    assert.deepEqual(page.data.snapshot?.value.font.family, ['Menlo']);
  } finally {
    await page.unmount();
  }
});

test('another writer moving the revision is retried once on top of it', async () => {
  const host = fakeTerminal({ conflicts: 1 });
  const page = await open(host);
  try {
    await page.actions.setFont({ ligatures: false });
    assert.equal(page.data.snapshot?.value.font.ligatures, false);
    assert.equal(page.data.snapshot?.revision, host.snapshot().revision);
  } finally {
    await page.unmount();
  }
});

test('a second conflict in a row is reported, not retried forever', async () => {
  const host = fakeTerminal({ conflicts: 2 });
  const page = await open(host);
  try {
    await assert.rejects(page.actions.setFont({ size: 20 }), (error: LxError) => error.reason === 'revisionOutdated');
  } finally {
    await page.unmount();
  }
});

test('quick changes are written one after another, never racing', async () => {
  const host = fakeTerminal();
  const page = await open(host);
  try {
    await Promise.all([page.actions.setFont({ size: 14 }), page.actions.setFont({ lineHeight: 1.2 }), page.actions.setFont({ ligatures: false })]);
    assert.equal(host.calls.update.length, 3);
    assert.deepEqual(page.data.snapshot?.value.font, { family: ['JetBrains Mono'], size: 14, lineHeight: 1.2, ligatures: false });
  } finally {
    await page.unmount();
  }
});

test('a packaged scheme is imported before it is chosen, into the slot asked for', async () => {
  const host = fakeTerminal({ appearance: 'dark' });
  const page = await open(host);
  try {
    await page.actions.setScheme({ name: 'nord', slot: 'dark' });
    assert.deepEqual(host.calls.import, ['nord']);
    assert.equal(page.data.snapshot?.value.theme.dark, 'nord');
    assert.equal(page.data.snapshot?.value.theme.light, 'lingxia-light');
    assert.equal(page.data.schemes.find((s) => s.name === 'nord')?.installed, true);
    await page.actions.setScheme({ name: 'nord', slot: 'dark' });
    assert.deepEqual(host.calls.import, ['nord'], 'imported once');
    await page.actions.setScheme({ name: 'lingxia-dark', slot: 'light' });
    assert.equal(page.data.snapshot?.value.theme.light, 'lingxia-dark', 'the slot not in use is editable too');
    assert.equal(page.data.snapshot?.value.theme.dark, 'nord');
  } finally {
    await page.unmount();
  }
});

test('reset returns every field to the defaults', async () => {
  const host = fakeTerminal();
  const page = await open(host);
  try {
    await page.actions.setFont({ size: 20 });
    await page.actions.resetAll();
    assert.equal(page.data.snapshot?.value.font.size, 13);
  } finally {
    await page.unmount();
  }
});

test('an imported file joins the list under its own name', async () => {
  const host = fakeTerminal();
  const page = await open(host);
  try {
    assert.deepEqual(await page.actions.importScheme(), { status: 'imported', name: 'Gruvbox' });
    assert.ok(page.data.schemes.some((s) => s.name === 'gruvbox' && s.source === 'imported'));
  } finally {
    await page.unmount();
  }
});

test('schemes sort into light and dark by their background, and keep packaged credits', () => {
  assert.equal(toneOf({ ...light.scheme }), 'light');
  assert.equal(toneOf({ ...dark.scheme }), 'dark');
  assert.equal(toneOf({ ...dark.scheme, background: 'not a colour' }), 'dark');
  const merged = mergeSchemes([{ name: 'nord', source: 'imported', scheme: PACKAGED[1].scheme }, { name: 'my-theme', source: 'imported', scheme: { ...dark.scheme, name: '' } }]);
  const nord = merged.find((s) => s.name === 'nord');
  assert.equal(nord?.source, 'packaged');
  assert.equal(nord?.author, 'Sven Greb');
  assert.equal(nord?.installed, true);
  assert.equal(merged.find((s) => s.name === 'my-theme')?.label, 'My Theme');
  assert.equal(merged.find((s) => s.name === 'dracula')?.installed, false);
});
