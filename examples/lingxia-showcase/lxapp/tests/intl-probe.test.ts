import { expect, test } from '@lingxia/test';
import { runtimePlatform } from '@lingxia/test/showcase/helpers/platform.js';

// Records which `Intl` features the Logic engine has (ArkJS on Harmony, JavaScriptCore on Apple,
// QuickJS on Android and Windows), one line per feature `format` relies on, without asserting:
// the attachment is compared across engines by hand.
test('record the Logic engine\'s Intl features', {
  id: 'INTL-PROBE-001',
  start: { page: 'home' },
}, async (t) => {
  const platform = await runtimePlatform(t.app);
  const lines = await t.app.logic.eval(() => {
    const out: string[] = [];
    const item = (name: string, run: () => unknown) => {
      let value: string;
      try {
        const result = run();
        value = typeof result === 'string' ? JSON.stringify(result) : String(result);
      } catch (error) {
        const failure = error as { name?: string; message?: string } | null;
        value = `throws ${failure?.name ?? typeof error}: ${failure?.message ?? ''}`;
      }
      out.push(`${name} | ${value}`);
    };
    const at = Date.UTC(2026, 10, 24, 2, 5, 30);
    // `Intl.ListFormat` is ES2021, newer than the tests' lib.
    const ListFormat = (Intl as unknown as { ListFormat: new (language: string, options?: object) => { format(items: string[]): string } }).ListFormat;
    const unit = (unitName: string, value: number) =>
      new Intl.NumberFormat('en-US', { style: 'unit', unit: unitName, unitDisplay: 'short', maximumFractionDigits: 1 }).format(value);
    item('Intl', () => typeof Intl);
    for (const name of ['NumberFormat', 'DateTimeFormat', 'ListFormat', 'RelativeTimeFormat', 'PluralRules', 'DurationFormat', 'Locale', 'Collator']) {
      item(`Intl.${name}`, () => typeof (Intl as unknown as Record<string, unknown>)[name]);
    }
    item('Intl.supportedValuesOf', () => typeof (Intl as unknown as { supportedValuesOf?: unknown }).supportedValuesOf);
    item('NumberFormat decimal en-US', () => new Intl.NumberFormat('en-US').format(1234.5678));
    item('NumberFormat percent en-US', () => new Intl.NumberFormat('en-US', { style: 'percent' }).format(0.256));
    item('NumberFormat signDisplay exceptZero', () => new Intl.NumberFormat('en-US', { signDisplay: 'exceptZero' }).format(5));
    item('NumberFormat useGrouping false', () => new Intl.NumberFormat('en-US', { useGrouping: false }).format(12345));
    item('NumberFormat ar-EG digits', () => new Intl.NumberFormat('ar-EG').format(123));
    for (const name of ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte', 'petabyte']) item(`unit ${name}`, () => unit(name, 1.5));
    for (const name of ['bit-per-second', 'kilobit-per-second', 'megabit-per-second', 'gigabit-per-second', 'terabit-per-second']) item(`unit ${name}`, () => unit(name, 487));
    for (const name of ['day', 'hour', 'minute', 'second']) item(`unit ${name} long`, () => new Intl.NumberFormat('en-US', { style: 'unit', unit: name, unitDisplay: 'long' }).format(2));
    const date = (options: Intl.DateTimeFormatOptions, language = 'en-US') =>
      new Intl.DateTimeFormat(language, { timeZone: 'Asia/Shanghai', ...options }).format(at);
    for (const style of ['short', 'medium', 'long', 'full'] as const) item(`dateStyle ${style}`, () => date({ dateStyle: style }));
    item('dateStyle medium + timeStyle short, hour12', () => date({ dateStyle: 'medium', timeStyle: 'short', hour12: true }));
    item('dateStyle medium + timeStyle short, h23', () => date({ dateStyle: 'medium', timeStyle: 'short', hourCycle: 'h23' }));
    item('time hour12', () => date({ hour: 'numeric', minute: '2-digit', hour12: true }));
    item('time h23', () => date({ hour: 'numeric', minute: '2-digit', hourCycle: 'h23' }));
    item('time ja hour12', () => date({ hour: 'numeric', minute: '2-digit', hour12: true }, 'ja'));
    item('weekday month day', () => date({ weekday: 'short', month: 'short', day: 'numeric' }));
    item('month year', () => date({ year: 'numeric', month: 'long' }));
    item('weekday time', () => date({ weekday: 'short', hour: 'numeric', minute: '2-digit', hour12: true }));
    item('timeZone Asia/Jerusalem', () => new Intl.DateTimeFormat('en-US', { timeZone: 'Asia/Jerusalem', hour: 'numeric', minute: '2-digit', hourCycle: 'h23' }).format(at));
    item('timeZone unknown', () => new Intl.DateTimeFormat('en-US', { timeZone: 'Mars/Base' }).format(at));
    item('formatToParts', () => new Intl.DateTimeFormat('en-US', { timeZone: 'UTC', hourCycle: 'h23', weekday: 'short', hour: 'numeric' }).formatToParts(at).map(part => part.type).join(','));
    item('ListFormat conjunction', () => new ListFormat('en-US').format(['a', 'b', 'c']));
    item('ListFormat disjunction es', () => new ListFormat('es', { type: 'disjunction' }).format(['siete', 'ocho']));
    item('ListFormat unit short', () => new ListFormat('en-US', { type: 'unit', style: 'short' }).format(['2 hr', '20 min']));
    item('ListFormat he', () => new ListFormat('he').format(['א', 'ב']));
    item('RelativeTimeFormat auto', () => new Intl.RelativeTimeFormat('en-US', { numeric: 'auto' }).format(-1, 'day'));
    item('RelativeTimeFormat short', () => new Intl.RelativeTimeFormat('en-US', { style: 'short' }).format(3, 'hour'));
    item('RelativeTimeFormat -0', () => new Intl.RelativeTimeFormat('en-US').format(-0, 'second'));
    item('supportedValuesOf timeZone', () => (Intl as unknown as { supportedValuesOf: (key: string) => string[] }).supportedValuesOf('timeZone').length);
    item('default locale', () => new Intl.DateTimeFormat().resolvedOptions().locale);
    item('default timeZone', () => new Intl.DateTimeFormat().resolvedOptions().timeZone);
    return out;
  });
  await t.attach(`intl-${platform}`, lines.join('\n'));
  expect(lines.length).toBeGreaterThan(40);
});
