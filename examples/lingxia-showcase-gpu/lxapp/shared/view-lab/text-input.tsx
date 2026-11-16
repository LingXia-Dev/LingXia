// Text & input demos of the View catalogue: each is the body of one demo.
import { Fragment, useId, useLocal, useView } from '@lingxia/view';
import { Button } from '../ui';
import { Readout, useSay } from './demo';
import { labels, readouts } from './text';
import type { LabPageData } from '../lab-types';

export function TextLayoutDemo() {
  return (
    <>
      <view className="flex-row gap-6">
        <view className="items-end">
          {['1,111.10', '808.08', '10.00'].map(value => <text key={value} className="text-sm tabular-nums slashed-zero">{value}</text>)}
        </view>
        <text data-testid="lab-text-pre-line" className="grow basis-0 text-sm whitespace-pre-line">{'First line\n   second line keeps   one space'}</text>
      </view>
      <text data-testid="lab-text-pre-wrap" className="p-2 rounded bg-surface-sunken text-xs font-mono whitespace-pre-wrap">{'{\n  "indent": true\n}'}</text>
      <text data-testid="lab-text-break-all" className="text-xs break-all">https://example.com/a/very/long/path/without/any/spaces/to/break/at</text>
    </>
  );
}

export function RichTextDemo() {
  const presses = useLocal('presses', 0);
  const t = useSay();
  return (
    <>
      {t({ en: '', zh: 'zh' }) ? (
        <text data-testid="lab-rich-paragraph" className="text-sm leading-relaxed">
          套餐<span className="font-semibold text-text">按月</span>续订，除非你先<span className="bg-surface-sunken font-mono">取消</span>。请在
          <span className="italic line-through text-muted">签署</span>同意之前阅读
          <link data-testid="lab-rich-link" onClick={() => presses.set(presses.value + 1)} className="active:text-text">服务条款与<span className="font-semibold">隐私政策</span></link>。
        </text>
      ) : (
        <text data-testid="lab-rich-paragraph" className="text-sm leading-relaxed">
          Plans renew <span className="font-semibold text-text">monthly</span> unless you{' '}
          <span className="bg-surface-sunken font-mono">cancel</span> first. Read the{' '}
          <link data-testid="lab-rich-link" onClick={() => presses.set(presses.value + 1)} className="active:text-text">terms of service and <span className="font-semibold">privacy policy</span></link>
          {' '}before you <span className="italic line-through text-muted">sign</span> agree.
        </text>
      )}
      <Readout testId="lab-rich-presses" state={`Link pressed ${presses.value} times`}>{t(readouts.presses(presses.value))}</Readout>
    </>
  );
}

// A long document: numbered clauses from Logic, inline runs, a clamped preview and a definitions grid.
export function DocumentDemo() {
  const { terms } = useView().data as unknown as LabPageData;
  const t = useSay();
  return (
    <>
      <view className="gap-4 max-w-[68ch]">
        <text className="text-xs text-muted tabular-nums">{`${t(labels.updated)} ${terms.updated}`}</text>
        {terms.clauses.map((clause, index) => (
          <view key={clause.heading} className="gap-2">
            <text role="heading" headingLevel={3} className="text-base font-semibold">{`${index + 1}. ${clause.heading}`}</text>
            <text className="text-sm leading-relaxed select-text">
              {clause.body} <span className="font-semibold">{clause.term}</span> is defined in{' '}
              <span className="font-mono text-xs bg-surface-sunken">section {index + 2}</span> and its <span className="italic">annexes</span>.
            </text>
            <text className="text-xs text-faint line-clamp-2">{clause.body.repeat(3)}</text>
          </view>
        ))}
        <text role="heading" headingLevel={3} className="text-base font-semibold">{t(labels.definitions)}</text>
        <view data-testid="lab-document-terms" className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1">
          {terms.clauses.map(clause => <Fragment key={clause.term}><Term word={clause.term} meaning={clause.heading} /></Fragment>)}
        </view>
      </view>
    </>
  );
}

export function SelectionDemo() {
  return (
    <>
      <text data-testid="lab-select-text" className="text-sm select-text">Long-press a word to select it, then drag the handles. Order reference: LX-2024-88412.</text>
      <text data-testid="lab-select-all" className="font-mono text-sm px-2 py-1 rounded bg-surface-sunken select-all">8F3A-77C2-91D0</text>
      <view className="select-text"><text data-testid="lab-select-inherited" className="text-xs text-muted">Selectable by inheritance.</text></view>
    </>
  );
}

// A typeahead: suggestions sit under the field in a manual popover while the caret stays in the
// text. Arrows move the active option, Enter takes it, Escape closes; a tap on one takes it too.
export function TypeaheadDemo() {
  const fruits = ['Apple', 'Apricot', 'Banana', 'Cherry', 'Grape'];
  const query = useLocal('lab-type-query', '');
  const open = useLocal('lab-type-open', false);
  const active = useLocal('lab-type-active', 0);
  const last = useLocal('lab-type-last', 'none');
  const matches = fruits.filter(name => name.toLowerCase().startsWith(query.value.toLowerCase()));
  const shown = open.value && matches.length > 0;
  const take = (name: string) => { query.set(name); open.set(false); last.set(`picked ${name}`); };
  const t = useSay();
  return (
    <>
      <view id="lab-type-field" data-testid="lab-type-field" className="rounded-lg border border-line-strong bg-surface">
        <input data-testid="lab-type-input" value={query.value} aria-label="Fruit" placeholder={t(labels.fruit)} className="px-3 py-2 text-sm"
          role="combobox" aria-expanded={shown} aria-controls="lab-type-list" aria-autocomplete="list"
          aria-activedescendant={shown ? `lab-type-option-${active.value}` : undefined}
          onInput={text => { query.set(text); open.set(true); active.set(0); }}
          onFocus={() => { open.set(true); last.set('focus'); }}
          onBlur={() => { open.set(false); last.set('blur'); }}
          onSubmit={() => last.set('submit')}
          onKeyDown={{
            ArrowDown: () => active.set(Math.min(matches.length - 1, active.value + 1)),
            ArrowUp: () => active.set(Math.max(0, active.value - 1)),
            Enter: () => { if (matches[active.value]) take(matches[active.value]); },
            Escape: () => { open.set(false); last.set('escape'); },
          }} />
      </view>
      <view id="lab-type-list" popover="manual" anchor="lab-type-field" open={shown} onToggle={next => open.set(next)}
        role="listbox" aria-label="Fruits" data-testid="lab-type-list" className="w-40 rounded-lg border border-line-strong bg-surface p-1">
        {matches.map((name, index) => (
          <button key={name} id={`lab-type-option-${index}`} role="option" aria-selected={index === active.value}
            data-testid={`lab-type-option-${name}`} onClick={() => take(name)}
            className={`px-3 py-2 rounded-md text-sm ${index === active.value ? 'bg-surface-sunken' : ''}`}>{name}</button>
        ))}
      </view>
      <Readout testId="lab-type-state" state={`open=${shown} active=${active.value} last=${last.value}`}>{t(readouts.suggestions(shown, active.value, last.value))}</Readout>
      <input data-testid="lab-type-readonly" value="Read only" readOnly aria-label="Reference"
        className="px-3 py-2 rounded-lg border border-line-strong bg-surface-sunken text-sm" />
    </>
  );
}

// A field's hint, error, required and invalid states, a tooltip, and a dialog that interrupts.
export function FormSemanticsDemo() {
  const zip = useLocal('form-zip', '');
  const confirm = useLocal('form-confirm', false);
  const hint = useId();
  const error = useId();
  const invalid = zip.value.length > 0 && !/^\d{5}$/.test(zip.value);
  return (
    <>
      <view role="group" aria-label="Delivery" data-testid="lab-form-group" className="gap-1">
        <input data-testid="lab-form-zip" bind={zip} aria-label="ZIP code" aria-required aria-invalid={invalid}
          aria-describedby={invalid ? `${hint} ${error}` : hint} placeholder="12345"
          className={`px-3 py-2 rounded-lg border bg-surface text-sm ${invalid ? 'border-danger' : 'border-line-strong'}`} />
        <text id={hint} className="text-xs text-muted">Five digits</text>
        {invalid ? <text id={error} data-testid="lab-form-error" className="text-xs text-danger">Not a ZIP code</text> : null}
        <text role="tooltip" data-testid="lab-form-tip" className="text-xs text-muted">We deliver within the city</text>
      </view>
      <Button testId="lab-form-delete" variant="secondary" onClick={() => confirm.set(true)}>Delete address</Button>
      <dialog open={confirm.value} role="alertdialog" onClose={() => confirm.set(false)} aria-label="Delete this address?" data-testid="lab-form-confirm"
        className="w-72 p-4 gap-3 rounded-2xl bg-surface-raised">
        <text className="text-sm">It cannot be restored.</text>
        <Button testId="lab-form-keep" onClick={() => confirm.set(false)}>Keep it</Button>
      </dialog>
    </>
  );
}

export function Term({ word, meaning }: { word: string; meaning: string }) {
  return <><text className="text-sm font-semibold">{word}</text><text className="text-xs text-muted">{meaning}</text></>;
}

// A parent box over three: it is checked when all are, mixed when some are.
export function PartlyDemo() {
  const picked = useLocal<readonly boolean[]>('lab-partly-picked', [false, false, false]);
  const t = useSay();
  const names = t(labels.toppings).split('|');
  const count = picked.value.filter(Boolean).length;
  const all = count === names.length;
  return (
    <>
      <view className="flex-row items-center gap-3">
        <checkbox data-testid="lab-partly-all" value={all} indeterminate={count > 0 && !all} aria-label={t(labels.allToppings)}
          onChange={next => picked.set(names.map(() => next))} />
        <text className="text-sm font-medium">{t(labels.allToppings)}</text>
      </view>
      <view className="ps-9 gap-2">
        {names.map((name, index) => (
          <view key={name} className="flex-row items-center gap-3">
            <checkbox data-testid={`lab-partly-${index}`} value={picked.value[index]} aria-label={name}
              onChange={next => picked.set(picked.value.map((was, at) => (at === index ? next : was)))} />
            <text className="text-sm">{name}</text>
          </view>
        ))}
      </view>
      <Readout testId="lab-partly-state" state={`checked=${count} of=${names.length}`}>{t(readouts.partly(count, names.length))}</Readout>
    </>
  );
}

// A link that takes no press beside one that does.
export function LinkOffDemo() {
  const taps = useLocal('lab-link-taps', 0);
  const t = useSay();
  return (
    <>
      <text data-testid="lab-link-paragraph" className="text-sm leading-relaxed">
        {t(labels.linkOffLead)}
        <link data-testid="lab-link-disabled" disabled onClick={() => taps.set(taps.value + 100)} className="text-faint">{t(labels.linkOffOld)}</link>
        {t(labels.linkOffMid)}
        <link data-testid="lab-link-enabled" onClick={() => taps.set(taps.value + 1)}>{t(labels.linkOffNew)}</link>.
      </text>
      <Readout testId="lab-link-state" state={`taps=${taps.value}`}>{t(readouts.taps(taps.value))}</Readout>
    </>
  );
}

/** The Text & input group: every demo the text table names, drawn by its body. */
// Decoration lines in their own style and colour: on a term, on a link and through a struck word.
export function UnderlinesDemo() {
  const t = useSay();
  return (
    <text data-testid="lab-underlines-text" className="text-base leading-relaxed">
      {t(labels.underLead)}<span data-testid="lab-underlines-term" className="underline decoration-dotted decoration-accent/60">{t(labels.underTerm)}</span>
      {t(labels.underMid)}<link data-testid="lab-underlines-link" onClick={() => {}} className="decoration-dashed decoration-danger">{t(labels.underLink)}</link>
      {t(labels.underEnd)}<span className="line-through decoration-muted">{t(labels.underStruck)}</span>.
    </text>
  );
}

// Two thumbs on one track: the pair while it moves, and the pair where the press ended.
export function RangeDemo() {
  const range = useLocal<readonly [number, number]>('lab-range-values', [20, 60]);
  const done = useLocal('lab-range-done', 'none');
  const t = useSay();
  return (
    <>
      <slider data-testid="lab-range-price" aria-label={t(labels.rangeName)} thumbLabels={[t(labels.rangeLow), t(labels.rangeHigh)]} min={0} max={100} step={10}
        values={range.value} onChange={next => range.set(next)} onChangeEnd={next => done.set(`${next[0]}-${next[1]}`)} className="text-accent" />
      <Readout testId="lab-range-state" state={`low=${range.value[0]} high=${range.value[1]} done=${done.value}`}>{t(readouts.range(range.value[0], range.value[1], done.value))}</Readout>
    </>
  );
}

export const textDemos = {
  paragraphs: TextLayoutDemo,
  runs: RichTextDemo,
  document: DocumentDemo,
  selecting: SelectionDemo,
  suggestions: TypeaheadDemo,
  partly: PartlyDemo,
  underlines: UnderlinesDemo,
  range: RangeDemo,
  linkOff: LinkOffDemo,
  form: FormSemanticsDemo,
};
