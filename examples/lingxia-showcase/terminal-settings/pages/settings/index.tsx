import { t, useAction, useEnv, useLocal } from '@lingxia/view';
import { Banner } from '@liulidesign/core/Banner';
import { Button } from '@liulidesign/core/Button';
import { ClickableCard, ClickableCardGroup } from '@liulidesign/core/Card';
import { Dialog } from '@liulidesign/core/Dialog';
import { EmptyState } from '@liulidesign/core/EmptyState';
import { Item, ItemList } from '@liulidesign/core/Item';
import { NumberInput } from '@liulidesign/core/NumberInput';
import { ProgressBar } from '@liulidesign/core/ProgressBar';
import { Section } from '@liulidesign/core/Section';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { Select } from '@liulidesign/core/Select';
import { Skeleton, SkeletonGroup } from '@liulidesign/core/Skeleton';
import { Slider } from '@liulidesign/core/Slider';
import { Switch } from '@liulidesign/core/Switch';
import { ToastRegion, useToast } from '@liulidesign/core/Toast';
import type { View } from './$types';

type Data = View['data'];
type Snapshot = NonNullable<Data['snapshot']>;
type Scheme = Data['schemes'][number];
type Slot = 'light' | 'dark';

const RAMP = ['red', 'yellow', 'green', 'cyan', 'blue', 'purple'] as const;

/** A tiny terminal in the scheme's own hex colours: two lines of text, then its colour ramp. Vectors are where page colours may be literal. */
function swatchPaths(colors: Scheme['scheme']) {
  const bar = (x: number, y: number, w: number) => `M${x} ${y}h${w}v4h${-w}Z`;
  const dot = (cx: number) => `M${cx - 3} 30a3 3 0 1 0 6 0a3 3 0 1 0 -6 0Z`;
  // An imported scheme's colours are unchecked strings; a non-hex one paints nothing.
  const paint = (value: string | undefined): `#${string}` | 'none' => (isHex(value) ? value : 'none');
  return [
    { d: 'M0 0H100V40H0Z', fill: paint(colors.background) },
    { d: bar(10, 7, 6), fill: paint(colors.green || colors.foreground) },
    { d: bar(20, 7, 48), fill: paint(colors.foreground) },
    { d: bar(10, 16, 72), fill: paint(colors.foreground) },
    ...RAMP.map((key, i) => ({ d: dot(13 + i * 14.8), fill: paint(colors[key] || colors.foreground) })),
  ];
}

function isHex(value: string | undefined): value is `#${string}` {
  return value !== undefined && /^#(?:[0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value);
}

const SIZE_MIN = 4;
const SIZE_MAX = 96;

export function render(view: View) {
  return (
    <page data-testid="terminal-settings" className="bg-canvas text-text">
      <ToastRegion>
        <Settings {...view} />
      </ToastRegion>
    </page>
  );
}

function Settings({ data, actions }: View) {
  if (data.status === 'unavailable') {
    return (
      <view className="grow justify-center px-6">
        <EmptyState title={t('app.unavailable')} description={t('app.unavailableHint')} data-testid="settings-unavailable" />
      </view>
    );
  }
  if (!data.snapshot) {
    return (
      <view className="w-full max-w-[720px] self-center px-6 py-6 gap-6">
        <SkeletonGroup label={t('app.loading')}>
          <Skeleton shape="block" className="h-40 rounded-container" />
          <Skeleton shape="block" className="h-56 rounded-container" />
        </SkeletonGroup>
      </view>
    );
  }
  const snapshot = data.snapshot;
  return (
    <scroll className="grow">
      <view className="w-full max-w-[720px] self-center px-6 py-6 gap-8">
        {snapshot.warnings.length > 0 ? (
          <Banner
            title={t('app.warning')}
            message={snapshot.warnings.map((warning) => warning.message).join(' ')}
            tone="warning"
            data-testid="settings-warnings"
          />
        ) : null}
        <Appearance snapshot={snapshot} schemes={data.schemes} actions={actions} />
        <Type snapshot={snapshot} fonts={data.fonts} actions={actions} />
        {data.inlineImages ? <InlineImages data={data} actions={actions} /> : null}
        <Reset actions={actions} />
      </view>
    </scroll>
  );
}

function sourceOf(scheme: Scheme): string {
  if (scheme.source === 'packaged') return scheme.spdx ? `${scheme.author} · ${scheme.spdx}` : scheme.author ?? '';
  return t(scheme.source === 'built_in' ? 'appearance.builtIn' : 'appearance.importedSource');
}

function Appearance({ snapshot, schemes, actions }: { snapshot: Snapshot; schemes: Data['schemes']; actions: View['actions'] }) {
  const inUse = snapshot.effective.appearance;
  const slotCell = useLocal<Slot>('schemeSlot', inUse);
  const slot = slotCell.value;
  const saved = snapshot.value.theme[slot];
  // A slot lists schemes of its own tone, plus whatever it holds now.
  const shown = schemes.filter((scheme) => scheme.tone === slot || scheme.name === saved);
  const env = useEnv();
  // Two cards a row only while each stays about 16 characters of body text wide.
  const columns = `repeat(auto-fill,minmax(${Math.round(144 * env.textScale)}px,1fr))`;
  // The card pressed last stays chosen while its write is in flight.
  const chosen = useLocal<string | null>('chosenScheme', null);
  const setScheme = useAction(actions.setScheme);
  const importScheme = useAction(actions.importScheme);
  const toast = useToast();

  const choose = async (name: string) => {
    chosen.set(name);
    await setScheme.run({ name, slot });
    chosen.set(null);
  };
  const runImport = async () => {
    const result = await importScheme.run();
    if (result.ok && result.value?.status === 'imported') {
      toast.show({ message: t('appearance.imported', { name: result.value.name }), tone: 'success', id: 'import' });
    }
  };
  const importError = importScheme.error
    ? t(importScheme.error.code === 'E_INVALID_DATA'
      ? 'appearance.importInvalid'
      : importScheme.error.code === 'E_ALREADY_EXISTS' ? 'appearance.importExists' : 'appearance.importFailed')
    : null;

  return (
    <Section
      title={t('appearance.title')}
      action={(
        <Button
          label={t('appearance.import')}
          size="sm"
          startIcon="add"
          isLoading={importScheme.pending}
          clickAction={runImport}
          data-testid="scheme-import"
        />
      )}
      data-testid="settings-appearance"
    >
      <view className="gap-1.5">
        <SegmentedControl
          label={t('appearance.slot')}
          items={[
            { value: 'light', label: t('appearance.slot.light') },
            { value: 'dark', label: t('appearance.slot.dark') },
          ]}
          value={slot}
          onChange={(value) => { slotCell.set(value as Slot); chosen.set(null); }}
          data-testid="scheme-slot"
        />
        <text className="text-footnote text-muted" data-testid="scheme-slot-caption">
          {t(slot === inUse ? 'appearance.slot.inUse' : slot === 'light' ? 'appearance.slot.usedLight' : 'appearance.slot.usedDark')}
        </text>
      </view>
      {shown.length === 0 ? (
        <text className="text-body text-muted">{t('appearance.empty')}</text>
      ) : (
        <ClickableCardGroup
          label={t('appearance.scheme')}
          value={chosen.value ?? (shown.some((scheme) => scheme.name === saved) ? saved : null)}
          onChange={(name) => { void choose(name); }}
          data-testid="schemes"
        >
          <view className="grid gap-3" style={{ gridTemplateColumns: columns }}>
            {shown.map((scheme) => (
              <SchemeCard key={scheme.name} scheme={scheme} source={sourceOf(scheme)} />
            ))}
          </view>
        </ClickableCardGroup>
      )}
      {setScheme.error ? <text role="alert" className="text-footnote text-danger">{t('app.saveFailed')}</text> : null}
      {importError ? <text role="alert" className="text-footnote text-danger" data-testid="scheme-import-error">{importError}</text> : null}
    </Section>
  );
}

/** The scheme drawn in its own colours above its name; the swatch is decoration, the label names it. */
function SchemeCard({ scheme, source }: { scheme: Scheme; source: string }) {
  const colors = scheme.scheme;
  return (
    <ClickableCard
      value={scheme.name}
      label={t('appearance.card', { name: scheme.label, source })}
      padding="sm"
      data-testid={`scheme-${scheme.name}`}
    >
      <view aria-hidden className="w-full aspect-[5/2] overflow-hidden rounded-control border border-line">
        <vector className="size-full object-fill" viewBox={[0, 0, 100, 40]} paths={swatchPaths(colors)} />
      </view>
      <view className="gap-0.5 px-1">
        <text className="text-body font-medium text-text">{scheme.label}</text>
        <text className="text-footnote text-muted">{source}</text>
      </view>
    </ClickableCard>
  );
}

function lineHeightText(value: number): string {
  return t('type.lineHeightValue', { value: value.toFixed(2).replace(/0$/, '') });
}

function Type({ snapshot, fonts, actions }: { snapshot: Snapshot; fonts: Data['fonts']; actions: View['actions'] }) {
  const font = snapshot.value.font;
  const effective = snapshot.effective.font;
  // What the person is typing or dragging, until its write lands; then the saved value shows again.
  const size = useLocal<{ value: number | null } | null>('sizeDraft', null);
  const lineHeight = useLocal<number | null>('lineHeightDraft', null);
  const setFont = useAction(actions.setFont);

  const shownSize = size.value ? size.value.value : font.size;
  const isSizeValid = shownSize !== null && shownSize >= SIZE_MIN && shownSize <= SIZE_MAX;
  const commitSize = async (next: number | null) => {
    size.set({ value: next });
    if (next === null || next < SIZE_MIN || next > SIZE_MAX) return;
    const result = await setFont.run({ size: next });
    if (result.ok && size.value?.value === next) size.set(null);
  };
  const commitLineHeight = async (next: number) => {
    lineHeight.set(next);
    const result = await setFont.run({ lineHeight: next });
    if (result.ok) lineHeight.set(null);
  };
  const missing = effective.fellBack && effective.missing.length > 0
    ? t('type.familyMissing', { family: effective.missing[0], fallback: effective.family })
    : null;
  const shownLineHeight = lineHeight.value ?? font.lineHeight;

  return (
    <Section title={t('type.title')} data-testid="settings-type">
      <ItemList label={t('type.title')}>
        <Item
          label={t('type.family')}
          description={missing ?? t('type.familyHint')}
          endContent={(
            <Select
              label={t('type.family')}
              isLabelHidden
              size="sm"
              value={effective.family}
              items={fonts.map((family) => ({ value: family, label: family }))}
              onChange={(family) => { void setFont.run({ family }); }}
              className="w-48"
              data-testid="font-family"
            />
          )}
        />
        <Item
          label={t('type.size')}
          endContent={(
            <NumberInput
              label={t('type.sizeName')}
              isLabelHidden
              size="sm"
              value={shownSize}
              min={SIZE_MIN}
              max={SIZE_MAX}
              suffix={t('type.sizeUnit')}
              decrementLabel={t('type.smaller')}
              incrementLabel={t('type.larger')}
              status={isSizeValid ? undefined : { type: 'error', message: t('type.sizeRange') }}
              onChange={(next) => { void commitSize(next); }}
              className="w-36"
              data-testid="font-size"
            />
          )}
        />
        <Slider
          label={t('type.lineHeight')}
          description={t('type.lineHeightHint')}
          valueLabel={lineHeightText(shownLineHeight)}
          value={shownLineHeight}
          min={0.5}
          max={3}
          step={0.05}
          onChange={(next) => lineHeight.set(next)}
          onChangeEnd={(next) => { void commitLineHeight(next); }}
          variant="row"
          data-testid="line-height"
        />
        <Switch
          label={t('type.ligatures')}
          description={t('type.ligaturesHint')}
          value={font.ligatures}
          onChange={(ligatures) => { void setFont.run({ ligatures }); }}
          variant="row"
          data-testid="ligatures"
        />
      </ItemList>
      {setFont.error ? <text role="alert" className="text-footnote text-danger" data-testid="type-error">{t('app.saveFailed')}</text> : null}
    </Section>
  );
}

function megabytes(bytes: number): string {
  return t('windows.megabytes', { value: (bytes / (1024 * 1024)).toFixed(1) });
}

function InlineImages({ data, actions }: { data: Data; actions: View['actions'] }) {
  const setInlineImages = useAction(actions.setInlineImages);
  const progress = data.inlineImagesProgress;
  const total = progress?.total;
  return (
    <Section title={t('windows.title')} data-testid="settings-windows">
      <ItemList label={t('windows.title')}>
        <Switch
          label={t('windows.inlineImages')}
          description={t('windows.inlineImagesHint')}
          value={data.inlineImages?.enabled ?? false}
          isLoading={setInlineImages.pending}
          onChange={(enabled) => { void setInlineImages.run({ enabled }); }}
          variant="row"
          data-testid="inline-images"
        />
      </ItemList>
      {setInlineImages.pending ? (
        <ProgressBar
          label={t('windows.downloading')}
          progress={progress}
          valueLabel={progress && total ? t('windows.downloadingAmount', { received: megabytes(progress.loaded), total: megabytes(total) }) : undefined}
          data-testid="inline-images-progress"
        />
      ) : null}
      {setInlineImages.error ? <text role="alert" className="text-footnote text-danger">{t('windows.failed')}</text> : null}
    </Section>
  );
}

function Reset({ actions }: { actions: View['actions'] }) {
  const confirming = useLocal('confirmReset', false);
  const resetAll = useAction(actions.resetAll);
  const reset = async () => {
    confirming.set(false);
    await resetAll.run();
  };
  return (
    <view className="gap-2">
      <ItemList label={t('reset.title')}>
        <Item
          label={t('reset.title')}
          description={t('reset.hint')}
          endContent={(
            <Button
              label={t('reset.action')}
              variant="destructive"
              size="sm"
              isLoading={resetAll.pending}
              onClick={() => confirming.set(true)}
              data-testid="reset"
            />
          )}
        />
      </ItemList>
      {resetAll.error ? <text role="alert" className="text-footnote text-danger">{t('app.saveFailed')}</text> : null}
      <Dialog
        isOpen={confirming.value}
        onOpenChange={(next) => confirming.set(next)}
        label={t('reset.confirmTitle')}
        description={t('reset.confirmMessage')}
        size="sm"
        isAlert
        dismissOn="closeRequest"
        data-testid="reset-dialog"
        actions={(
          <>
            <Button label={t('common.cancel')} onClick={() => confirming.set(false)} data-testid="reset-cancel" />
            <Button label={t('reset.confirm')} variant="destructive" clickAction={reset} data-testid="reset-confirm" />
          </>
        )}
      />
    </view>
  );
}
