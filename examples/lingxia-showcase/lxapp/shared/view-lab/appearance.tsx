// Appearance & environment demos of the View catalogue: each is the body of one demo.
import { useLocal, useView } from '@lingxia/view';
import { Button, Icon } from '../ui';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';
import { ScreenDemo } from './screen';

// Counts this page's View runs: a theme change repaints a page that never read the theme
// without running it, and runs one that did.
let runs = 0;
export function ThemeDemo() {
  const reads = useLocal('lab-appearance-reads', false);
  const { env } = useView();
  const t = useSay();
  runs += 1;
  const theme = reads.value ? env.theme : null;
  return (
    <>
      <Readout testId="lab-appearance-runs" state={`runs=${runs} theme=${theme ?? 'unread'}`}>{t(readouts.theme(runs, theme))}</Readout>
      <view className="flex-row">
        <Button testId="lab-appearance-toggle" size="sm" variant="secondary" onClick={() => reads.set(!reads.value)}>
          {t(reads.value ? labels.stopTheme : labels.readTheme)}
        </Button>
      </view>
    </>
  );
}

export function DirectionDemo() {
  const t = useSay();
  return (
    <>
      {(['ltr', 'rtl'] as const).map(dir => (
        <view key={dir} dir={dir} data-testid={`lab-direction-${dir}`} className="flex-row items-center gap-2 ps-3 pe-1 py-2 rounded-s-xl border-s-4 border-accent bg-surface-sunken">
          <Icon name="chevron" className="w-5 h-5 text-text rtl:-scale-x-100" />
          <text className="grow basis-0 text-sm text-start">{dir === 'rtl' ? 'مرحبا بالعالم' : t(labels.hello)}</text>
          <text className="text-xs text-muted ms-auto">{dir}</text>
        </view>
      ))}
    </>
  );
}

// `data-*` reads the element's own props; `has-*` and `focus-within:` follow what is below it.
export function StateDemo() {
  const agreed = useLocal('variants-agreed', false);
  const open = useLocal('variants-open', false);
  const note = useLocal('variants-note', '');
  const t = useSay();
  return (
    <>
      <view data-testid="lab-variants-agree" className="flex-row items-center gap-3 rounded-xl px-4 py-3 bg-surface-sunken has-checked:bg-accent-solid has-checked:text-on-accent-solid">
        <checkbox data-testid="lab-variants-checkbox" bind={agreed} aria-label="Agree" />
        <text className="text-sm">{t(agreed.value ? labels.agreed : labels.agree)}</text>
      </view>
      <button data-testid="lab-variants-toggle" data-state={open.value ? 'open' : 'closed'} onClick={() => open.set(!open.value)}
        className="rounded-xl px-4 py-3 text-sm bg-surface-sunken data-[state=open]:bg-accent-solid data-[state=open]:text-on-accent-solid data-[state=open]:py-5">
        {t(open.value ? labels.openTaller : labels.closedShort)}
      </button>
      <view data-testid="lab-variants-field" className="rounded-xl p-3 gap-1 border border-line focus-within:border-accent focus-within:bg-surface-sunken">
        <text className="text-xs text-muted">{t(labels.note)}</text>
        <input data-testid="lab-variants-input" bind={note} placeholder={t(labels.focusMe)} aria-label="Note" className="text-sm" />
      </view>
    </>
  );
}

// What the page is told about the screen: the insets it keeps clear, and the keyboard over it.
export function InsetsDemo() {
  const { safeArea, keyboard } = useView().env;
  const t = useSay();
  return (
    <>
      <Readout testId="lab-insets-state" state={`top=${safeArea.top} bottom=${safeArea.bottom} keyboard=${keyboard.height}`}>
        {t(readouts.insets(Math.round(safeArea.top), Math.round(safeArea.bottom), Math.round(keyboard.height)))}
      </Readout>
      <input data-testid="lab-insets-field" aria-label="Keyboard" placeholder={t(labels.tapForKeyboard)}
        className="px-3 py-2 rounded-lg border border-line-strong bg-surface text-sm" />
    </>
  );
}

/** The Appearance & environment group: every demo the text table names, drawn by its body. */
export const appearanceDemos = {
  theme: ThemeDemo,
  direction: DirectionDemo,
  state: StateDemo,
  screen: ScreenDemo,
  insets: InsetsDemo,
};
