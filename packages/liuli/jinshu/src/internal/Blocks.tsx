import { cx, useBox, useLocal, useVideo, useView, type Child } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Button } from '@liulidesign/core/Button';
import { Details } from '@liulidesign/core/Details';
import { Lightbox } from '@liulidesign/core/Lightbox';
import { Skeleton, SkeletonGroup } from '@liulidesign/core/Skeleton';
import { VideoControls } from '@liulidesign/core/VideoControls';
import { plainText, tableColumnCount } from '../schema/helpers';
import { validateSrc } from '../schema/validate';
import type {
  AudioBlock,
  BlockProps,
  CalloutBlock,
  CodeBlock,
  ColumnsBlock,
  DetailsBlock,
  ExtensionBlock,
  FileBlock,
  FilePolicy,
  HeadingBlock,
  ImageBlock,
  JinshuBlock,
  ListBlock,
  MathBlock,
  MediaPolicy,
  ParagraphBlock,
  QuoteBlock,
  TableBlock,
  VideoBlock,
} from '../schema/types';
import { renderInlineRuns, type InlineAtomRenderer } from './InlineRuns';
import { MathFormulaView } from './MathView';
import { renderHighlightedCodeLine } from './codeHighlight';
import { jinshuText } from '../i18n/text';
import { blockElementId } from './nav';
import { jinshuViewRecipe as r } from '../JinshuView/JinshuView.recipe';

export type CustomBlockRenderer = (props: BlockProps<any>) => Child;

/** Shared, mutable count of `select-text` nodes still allowed this render (the View caps them per page). */
export interface SelectBudget {
  left: number;
}

export interface BlockRenderContext {
  readonly mode: 'view' | 'edit';
  /** The instance scope for block element ids; without it blocks carry no `id` (a diff's before/after pairs). */
  readonly idScope?: string;
  /** The instance's test id; block test ids hang under it, and without it blocks carry none. */
  readonly testScope?: string;
  readonly resolveSrc?: (src: string) => string;
  readonly filePolicy?: FilePolicy;
  readonly mediaPolicy?: MediaPolicy;
  readonly customBlocks?: Readonly<Record<string, CustomBlockRenderer>>;
  readonly customInlines?: Readonly<Record<string, InlineAtomRenderer>>;
  readonly isSelectable?: boolean;
  /** When set, each selectable text node takes one from it; past zero, text renders unselectable. */
  readonly selectBudget?: SelectBudget;
  readonly targetBlockId?: string | null;
  /** Playback positions to resume media at, by block id. */
  readonly resumeMedia?: Readonly<Record<string, { readonly positionMs: number }>>;
  readonly onLinkPress?: (info: { readonly blockId: string; readonly href: string }) => void;
  readonly onInPageLink?: (targetBlockId: string) => void;
  readonly onFileAction?: (info: { readonly blockId: string; readonly src: string; readonly action: 'download' }) => void;
  readonly onOpenImage?: (blockId: string) => void;
  readonly onCopyCode?: (code: string, blockId: string) => void;
  readonly onEmitBlockEvent?: (blockId: string, type: string, data: unknown) => void;
  readonly onPlayedSegment?: (blockId: string, src: string, range: [number, number], durationMs: number, rate: number, ended?: true) => void;
  readonly onToggleTask?: (blockId: string, itemId: string, checked: boolean) => void;
  readonly onUpdateCustomAttrs?: (blockId: string, attrs: Record<string, unknown>) => void;
  readonly highlightedBlockIds?: ReadonlySet<string>;
}

const TICK_PATH = [
  { d: 'M3.5 8.5l3 3 6-7', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' },
] as const;

function safeResolveSrc(src: string, resolveSrc?: (s: string) => string): string | null {
  const resolved = resolveSrc ? resolveSrc(src) : src;
  const err = validateSrc(resolved);
  return err ? null : resolved;
}

function formatBytes(bytes?: number): string {
  if (bytes === undefined || bytes < 0) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatTimeMs(ms?: number): string {
  if (ms === undefined || ms < 0) return '0:00';
  const totalSec = Math.floor(ms / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec < 10 ? '0' : ''}${sec}`;
}

function fileExtLabel(block: FileBlock): string {
  const dot = block.name.lastIndexOf('.');
  if (dot >= 0 && dot < block.name.length - 1) {
    return block.name.slice(dot + 1).toUpperCase().slice(0, 6);
  }
  if (block.mimeType) {
    const slash = block.mimeType.indexOf('/');
    return (slash >= 0 ? block.mimeType.slice(slash + 1) : block.mimeType).toUpperCase().slice(0, 6);
  }
  return 'FILE';
}

function containsBlockId(blocks: readonly JinshuBlock[], targetId: string | null | undefined): boolean {
  if (!targetId) return false;
  for (const b of blocks) {
    if (b.id === targetId) return true;
    if ((b.type === 'quote' || b.type === 'callout' || b.type === 'details') && containsBlockId(b.blocks, targetId)) return true;
    if (b.type === 'list' && b.items.some(i => i.id === targetId || (i.blocks && containsBlockId(i.blocks, targetId)))) return true;
    if (b.type === 'columns' && b.columns.some(c => c.id === targetId || containsBlockId(c.blocks, targetId))) return true;
  }
  return false;
}

function blockTestId(blockId: string, ctx: BlockRenderContext): string | undefined {
  return ctx.testScope && `${ctx.testScope}-block-${blockId}`;
}

function blockDomId(blockId: string, ctx: BlockRenderContext): string | undefined {
  return ctx.idScope === undefined ? undefined : blockElementId(ctx.idScope, blockId);
}

/** Whether this text node may be selectable; consumes one from the page budget. */
function takeSelect(ctx: BlockRenderContext): boolean {
  if (!ctx.isSelectable) return false;
  if (!ctx.selectBudget) return true;
  if (ctx.selectBudget.left <= 0) return false;
  ctx.selectBudget.left -= 1;
  return true;
}

/** The gap above a block, by what precedes it: headings breathe more above than below. */
export function blockTopSpacing(prevType: string | null, currType: string): string | undefined {
  const s = r.spacing;
  if (prevType === null) return undefined;
  if (currType === 'heading') return prevType === 'heading' ? s.headingAfterHeading : s.heading;
  if (prevType === 'heading') return s.afterHeading;
  if (currType === 'divider' || prevType === 'divider') return s.divider;
  if (SPACIOUS.has(currType)) return s.spacious;
  return s.text;
}

const SPACIOUS = new Set(['callout', 'code', 'table', 'image', 'video', 'audio', 'file', 'columns', 'math']);

// LingXia caps one <text> at 256 runs; leave headroom for the trailing newlines.
const CODE_RUN_BUDGET = 220;

/** A code block's frame, shared by the reader and the editor: one header (file · language, then an action), numbered lines. */
export function CodeFrame({
  block,
  id,
  testId,
  isSelectable = false,
  action,
  className,
}: {
  block: Pick<CodeBlock, 'code' | 'language' | 'filename' | 'highlightLines'>;
  id?: string;
  testId?: string;
  isSelectable?: boolean;
  action?: Child;
  className?: string;
}) {
  const lines = block.code.split('\n');
  const marked = new Set(block.highlightLines ?? []);
  const hasMarks = marked.size > 0;

  // One <text> for the whole block (one selection, one select budget slot), coloured per token.
  let runsLeft = CODE_RUN_BUDGET;
  const body = lines.map((line, idx) => {
    const end = idx < lines.length - 1 ? '\n' : '';
    const highlighted = runsLeft > 8 ? renderHighlightedCodeLine(line, block.language, `l${idx}`) : line || ' ';
    const cost = Array.isArray(highlighted) ? highlighted.length + 1 : 1;
    runsLeft -= cost;
    return (
      <span key={`l-${idx}`}>
        {Array.isArray(highlighted) ? [...highlighted, end] : `${highlighted}${end}`}
      </span>
    );
  });

  return (
    <view id={id} data-testid={testId} className={cx(r.code.root, className)}>
      <view className={r.code.header}>
        <text className={r.code.lang}>{codeTitle(block)}</text>
        {action}
      </view>
      <scroll className={r.code.body}>
        <view className={r.code.lines}>
          {/* Each tint row is `h-[1.5em]` matching `leading-[1.5]` on the same `text-footnote` parent — zero drift at any textScale. */}
          {hasMarks && (
            <view aria-hidden className={r.code.marks}>
              {lines.map((_, idx) => (
                <view
                  key={`hl-${idx}`}
                  className={cx(r.code.mark, marked.has(idx + 1) && r.code.markOn)}
                />
              ))}
            </view>
          )}
          <CodeLineNumbers count={lines.length} testId={testId && `${testId}-numbers`} />
          <text className={cx(r.code.text, isSelectable && r.selectable)}>
            {body}
          </text>
        </view>
      </scroll>
    </view>
  );
}

/** The header title: file and language, or the language alone. */
export function codeTitle(block: Pick<CodeBlock, 'language' | 'filename'>): string {
  const language = block.language || jinshuText('@jinshu.view.code.plainText');
  return block.filename ? `${block.filename} · ${language}` : language;
}

/** The line-number gutter; the editor puts the same one beside its field. */
export function CodeLineNumbers({ count, testId }: { count: number; testId?: string }) {
  return (
    <text aria-hidden data-testid={testId} className={r.code.numbers}>
      {Array.from({ length: Math.max(1, count) }, (_, idx) => `${idx + 1}`).join('\n')}
    </text>
  );
}

function JinshuCodeBlockView({ block, ctx }: { block: CodeBlock; ctx: BlockRenderContext }) {
  const copied = useLocal(`copied-${block.id}`, false);
  return (
    <CodeFrame
      block={block}
      id={blockDomId(block.id, ctx)}
      testId={blockTestId(block.id, ctx)}
      isSelectable={takeSelect(ctx)}
      action={
        ctx.onCopyCode && (
          <Button
            label={copied.value ? jinshuText('@jinshu.view.code.copied') : jinshuText('@jinshu.view.code.copy')}
            startIcon={copied.value ? 'check' : 'copy'}
            variant="ghost"
            size="sm"
            onClick={() => {
              copied.set(true);
              ctx.onCopyCode?.(block.code, block.id);
            }}
          />
        )
      }
    />
  );
}

/** A table's frame, shared by the reader and the editor: wider than the frame, the grid scrolls sideways and fades at each edge it runs under. */
export function TableFrame({ table, id, testId, children }: { table: TableBlock; id?: string; testId?: string; children: Child }) {
  const blockId = table.id;
  const frame = useBox(`tbl-frame-${blockId}`);
  const grid = useBox(`tbl-grid-${blockId}`);
  const edges = useLocal(`tbl-edges-${blockId}`, { start: true, end: false });
  const room = frame.value?.width;
  const need = grid.value?.width;
  const overflows = room !== undefined && need !== undefined && need > room + 0.5;
  const under = !edges.value.start && !edges.value.end ? 'both' : !edges.value.start ? 'start' : !edges.value.end ? 'end' : undefined;
  return (
    <view id={id} data-testid={testId} box={frame} className={r.table.frame}>
      <scroll
        data-testid={testId && `${testId}-scroll`}
        className={cx(r.table.scroll, overflows && under && r.table.more[under])}
        onScrollEdge={({ edge, at }) => edges.set({ ...edges.value, [edge]: at })}
      >
        <view box={grid} className={r.table.grid} style={{ minWidth: tableMinWidth(table) }}>
          {children}
        </view>
      </scroll>
    </view>
  );
}

function JinshuImageBlockView({ block, ctx }: { block: ImageBlock; ctx: BlockRenderContext }) {
  const failed = useLocal(`img-err-${block.id}`, false);
  const retryCount = useLocal(`img-retry-${block.id}`, 0);
  const lightboxOpen = useLocal(`img-lb-${block.id}`, false);
  const resolved = safeResolveSrc(block.src, ctx.resolveSrc);
  const captionText = block.caption ? plainText(block.caption) : undefined;

  const sizeClass = r.image.size[block.size ?? 'full'];

  if (!resolved || failed.value) {
    return (
      <view id={blockDomId(block.id, ctx)} data-testid={blockTestId(block.id, ctx)} className={cx(sizeClass, r.media.errorBox)}>
        <text className={r.media.error}>{!resolved ? jinshuText('@jinshu.view.image.blocked') : jinshuText('@jinshu.view.image.failed')}</text>
        {resolved && (
          <Button
            label={jinshuText('@jinshu.view.retry')}
            size="sm"
            variant="tonal"
            onClick={() => {
              failed.set(false);
              retryCount.set(retryCount.value + 1);
            }}
          />
        )}
      </view>
    );
  }

  const aspectRatio = block.width && block.height && block.height > 0 ? block.width / block.height : undefined;

  return (
    <view id={blockDomId(block.id, ctx)} data-testid={blockTestId(block.id, ctx)} className={cx(sizeClass, r.image.figure)}>
      <button
        aria-label={block.alt || captionText || jinshuText('@jinshu.view.image.open')}
        onClick={() => {
          if (ctx.onOpenImage) ctx.onOpenImage(block.id);
          else lightboxOpen.set(true);
        }}
        className={r.image.frame}
        style={{ aspectRatio: aspectRatio ?? 4 / 3 }}
      >
        <image
          src={resolved}
          alt={block.alt ?? ''}
          retryKey={String(retryCount.value)}
          onError={() => failed.set(true)}
          className={r.image.picture}
          style={{ objectFit: aspectRatio ? 'cover' : 'contain' }}
        />
      </button>
      {block.caption && block.caption.length > 0 && (
        <text className={r.media.caption}>
          {renderInlineRuns(block.caption, {
            blockId: block.id,
            onLinkPress: ctx.onLinkPress,
            onInPageLink: ctx.onInPageLink,
            customInlines: ctx.customInlines,
          })}
        </text>
      )}
      {!ctx.onOpenImage && ctx.mode !== 'edit' && (
        <Lightbox
          isOpen={lightboxOpen.value}
          onOpenChange={open => lightboxOpen.set(open)}
          label={block.alt || jinshuText('@jinshu.view.image.preview')}
          images={[{ src: resolved, alt: block.alt ?? '', caption: captionText }]}
        />
      )}
    </view>
  );
}

/**
 * Real playback observation for a `<video>` (video and audio blocks): each played stretch is reported
 * once it ends (pause, end, seek, rate change), `seek: 'played-only'` snaps a forward seek back to the
 * furthest point already played, `maxRate` clamps the rate, and `resumeMedia` seeks on first metadata.
 */
function usePlaybackTracking(blockId: string, src: string, ctx: BlockRenderContext, player: ReturnType<typeof useVideo>, knownDurationMs?: number) {
  const segStartMs = useLocal<number | null>(`pb-seg-${blockId}`, null);
  const maxPlayedMs = useLocal(`pb-max-${blockId}`, 0);
  const lastMs = useLocal(`pb-last-${blockId}`, 0);
  const resumed = useLocal(`pb-resumed-${blockId}`, false);
  const durationMs = knownDurationMs ?? (player.duration !== null ? Math.round(player.duration * 1000) : 0);
  const policy = ctx.mediaPolicy;

  const closeSegment = (endMs: number, ended?: true) => {
    const start = segStartMs.value;
    segStartMs.set(null);
    if (start === null || endMs - start < 250) return;
    ctx.onPlayedSegment?.(blockId, src, [Math.round(start), Math.round(endMs)], durationMs || Math.round(endMs), player.playbackRate || 1, ended);
  };

  return {
    maxPlayedMs: maxPlayedMs.value,
    durationMs,
    handlers: {
      onLoadedMetadata: () => {
        const at = ctx.resumeMedia?.[blockId]?.positionMs;
        if (!resumed.value && at !== undefined && at > 0) {
          resumed.set(true);
          void player.seek(at / 1000);
        }
      },
      onPlay: () => segStartMs.set(player.currentTime * 1000),
      onTimeUpdate: (e: { readonly currentTime: number }) => {
        const ms = e.currentTime * 1000;
        // Played means reached from the previous update: a jump past it is a seek, not watching.
        const playedOn = ms - lastMs.value < 2000;
        lastMs.set(ms);
        if (segStartMs.value === null && !player.paused) segStartMs.set(ms);
        if (ms > maxPlayedMs.value && (playedOn || policy?.seek !== 'played-only')) maxPlayedMs.set(ms);
      },
      onPause: () => closeSegment(player.currentTime * 1000),
      onEnded: () => closeSegment(durationMs || player.currentTime * 1000, true),
      onSeeked: (e: { readonly currentTime: number }) => {
        const ms = e.currentTime * 1000;
        if (policy?.seek === 'none' || (policy?.seek === 'played-only' && ms > maxPlayedMs.value + 1000)) {
          void player.seek(maxPlayedMs.value / 1000);
          return;
        }
        closeSegment(lastMs.value);
        if (!player.paused) segStartMs.set(ms);
      },
      onRateChange: (e: { readonly playbackRate: number }) => {
        if (policy?.maxRate !== undefined && e.playbackRate > policy.maxRate) {
          void player.setPlaybackRate(policy.maxRate);
          return;
        }
        closeSegment(player.currentTime * 1000);
        if (!player.paused) segStartMs.set(player.currentTime * 1000);
      },
    },
  };
}

function ChapterRow({
  blockId,
  chapters,
  maxPlayedMs,
  ctx,
  player,
}: {
  blockId: string;
  chapters: readonly { readonly atMs: number; readonly title: string }[];
  maxPlayedMs: number;
  ctx: BlockRenderContext;
  player: ReturnType<typeof useVideo>;
}) {
  const nowMs = player.currentTime * 1000;
  return (
    <scroll className={r.chapters.base}>
      <view className={r.chapters.row}>
      {chapters.map((ch, idx) => {
        const blocked = ctx.mediaPolicy?.seek === 'none' || (ctx.mediaPolicy?.seek === 'played-only' && ch.atMs > maxPlayedMs + 1000);
        const isCurrent = nowMs >= ch.atMs && (idx === chapters.length - 1 || nowMs < chapters[idx + 1].atMs);
        return (
          <Button
            key={`${blockId}-ch-${idx}`}
            label={`${formatTimeMs(ch.atMs)} ${ch.title}`}
            size="sm"
            variant={isCurrent ? 'tonal' : 'ghost'}
            isDisabled={blocked}
            disabledMessage={jinshuText('@jinshu.view.media.notPlayedYet')}
            onClick={() => {
              void player.seek(ch.atMs / 1000);
              if (player.paused) void player.play();
            }}
          />
        );
      })}
      </view>
    </scroll>
  );
}

function JinshuVideoBlockView({ block, ctx }: { block: VideoBlock; ctx: BlockRenderContext }) {
  const player = useVideo(`jinshu-video-${block.id}`);
  const resolved = safeResolveSrc(block.src, ctx.resolveSrc);
  const poster = block.poster ? safeResolveSrc(block.poster, ctx.resolveSrc) ?? undefined : undefined;
  const tracking = usePlaybackTracking(block.id, block.src, ctx, player, block.durationMs);

  if (!resolved) {
    return (
      <view id={blockDomId(block.id, ctx)} className={r.media.errorBox}>
        <text className={r.media.error}>{jinshuText('@jinshu.view.video.blocked')}</text>
      </view>
    );
  }

  const label = block.caption ? plainText(block.caption) : jinshuText('@jinshu.view.video.label');
  return (
    <view id={blockDomId(block.id, ctx)} data-testid={blockTestId(block.id, ctx)} className={r.video.base}>
      <view className={r.video.frame}>
        <video player={player} src={resolved} poster={poster} aria-label={label} className={r.video.picture} {...tracking.handlers} />
        <VideoControls player={player} poster={poster} variant="overlay" data-testid={ctx.testScope && `${ctx.testScope}-video-${block.id}-controls`} />
      </view>
      {block.chapters && block.chapters.length > 0 && (
        <ChapterRow blockId={block.id} chapters={block.chapters} maxPlayedMs={tracking.maxPlayedMs} ctx={ctx} player={player} />
      )}
      {ctx.mediaPolicy?.seek === 'played-only' && <text className={r.video.seekNote}>{jinshuText('@jinshu.view.video.playedOnly')}</text>}
      {block.caption && block.caption.length > 0 && (
        <text className={r.media.caption}>
          {renderInlineRuns(block.caption, { blockId: block.id, onLinkPress: ctx.onLinkPress, onInPageLink: ctx.onInPageLink, customInlines: ctx.customInlines })}
        </text>
      )}
    </view>
  );
}

const PLAY_PATH = [{ d: 'M8.25 6.6v10.8a.75.75 0 0 0 1.14.64l8.6-5.4a.75.75 0 0 0 0-1.28l-8.6-5.4a.75.75 0 0 0-1.14.64z', fill: 'currentColor' }] as const;
const PAUSE_PATH = [{ d: 'M8 6.5h2v11H8zM14 6.5h2v11h-2z', fill: 'currentColor' }] as const;

/** A real audio player: the source plays through a `<video>` with no picture; controls are an audio card. */
function JinshuAudioBlockView({ block, ctx }: { block: AudioBlock; ctx: BlockRenderContext }) {
  const player = useVideo(`jinshu-audio-${block.id}`);
  const resolved = safeResolveSrc(block.src, ctx.resolveSrc);
  const tracking = usePlaybackTracking(block.id, block.src, ctx, player, block.durationMs);
  const durSec = player.duration ?? (block.durationMs ? block.durationMs / 1000 : 0);
  const title = block.title || jinshuText('@jinshu.view.audio.label');

  if (!resolved) {
    return (
      <view id={blockDomId(block.id, ctx)} className={r.media.errorBox}>
        <text className={r.media.error}>{jinshuText('@jinshu.view.audio.blocked')}</text>
      </view>
    );
  }

  const playing = !player.paused && !player.ended;
  const maxRate = ctx.mediaPolicy?.maxRate ?? 2;
  const rates = [1, 1.25, 1.5, 2].filter(rt => rt <= maxRate);
  const currentRate = player.playbackRate || 1;
  const nextRate = rates[(rates.indexOf(currentRate) + 1) % rates.length] ?? 1;

  return (
    <view id={blockDomId(block.id, ctx)} data-testid={blockTestId(block.id, ctx)} className={r.audio.card}>
      <video player={player} src={resolved} aria-label={title} className={r.audio.player} {...tracking.handlers} />
      <view className={r.audio.row}>
        <button
          aria-label={playing ? jinshuText('@jinshu.view.audio.pause', { title }) : jinshuText('@jinshu.view.audio.play', { title })}
          data-testid={ctx.testScope && `${ctx.testScope}-audio-${block.id}-toggle`}
          onClick={() => void (playing ? player.pause() : player.play())}
          className={r.audio.toggle}
        >
          <vector aria-hidden paths={playing ? PAUSE_PATH : PLAY_PATH} viewBox={[0, 0, 24, 24]} className={r.audio.toggleGlyph} />
        </button>
        <view className={r.audio.main}>
          <view className={r.audio.header}>
            <text className={r.audio.title}>{title}</text>
            <view className={r.audio.meta}>
              {rates.length > 1 && (
                <button
                  aria-label={jinshuText('@jinshu.view.audio.rate')}
                  onClick={() => void player.setPlaybackRate(nextRate)}
                  className={r.audio.rateButton}
                >
                  <text className={r.audio.rateText}>{`${currentRate}×`}</text>
                </button>
              )}
              <text className={r.audio.time}>
                {player.error ? jinshuText('@jinshu.view.media.cannotPlay') : player.buffering ? jinshuText('@jinshu.view.media.buffering') : `${formatTimeMs(player.currentTime * 1000)} / ${formatTimeMs(durSec * 1000)}`}
              </text>
            </view>
          </view>
          <slider
            aria-label={jinshuText('@jinshu.view.audio.progress', { title })}
            aria-valuetext={formatTimeMs(player.currentTime * 1000)}
            min={0}
            max={Math.max(1, durSec)}
            step={0.5}
            value={player.currentTime}
            disabled={durSec <= 0 || ctx.mediaPolicy?.seek === 'none'}
            onChangeEnd={v => void player.seek(v)}
            className={r.audio.slider}
          />
        </view>
      </view>
      {player.error && (
        <view className={r.audio.errorRow}>
          <text className={r.audio.error}>{player.error.message}</text>
          {player.error.recoverable && <Button label={jinshuText('@jinshu.view.retry')} size="sm" variant="tonal" onClick={() => void player.retry()} />}
        </view>
      )}
      {block.chapters && block.chapters.length > 0 && (
        <ChapterRow blockId={block.id} chapters={block.chapters} maxPlayedMs={tracking.maxPlayedMs} ctx={ctx} player={player} />
      )}
      {block.transcript && block.transcript.length > 0 && (
        <Details label={jinshuText('@jinshu.view.audio.transcript')}>
          <view className={r.audio.transcript}>{block.transcript.map((p, idx) => renderBlock(p, idx > 0 ? block.transcript![idx - 1].type : null, ctx))}</view>
        </Details>
      )}
    </view>
  );
}

function JinshuDetailsView({ block, ctx }: { block: DetailsBlock; ctx: BlockRenderContext }) {
  const open = useLocal(`det-open-${block.id}`, block.defaultOpen === true);
  // A navigation target inside forces it open, even when it was mounted closed.
  const forceOpen = containsBlockId(block.blocks, ctx.targetBlockId);
  return (
    <view id={blockDomId(block.id, ctx)} data-testid={blockTestId(block.id, ctx)}>
      <Details label={plainText(block.summary) || jinshuText('@jinshu.view.details')} isOpen={open.value || forceOpen} onOpenChange={next => open.set(next)}>
        <view className={r.details.body}>{block.blocks.map((child, idx) => renderBlock(child, idx > 0 ? block.blocks[idx - 1].type : null, ctx))}</view>
      </Details>
    </view>
  );
}

function JinshuFileBlockView({ block, ctx }: { block: FileBlock; ctx: BlockRenderContext }) {
  const effectiveDisplay =
    ctx.filePolicy?.inline === 'never'
      ? 'attachment'
      : block.display ?? ctx.filePolicy?.default ?? 'attachment';

  const metaParts: string[] = [];
  if (block.pageCount !== undefined) metaParts.push(jinshuText('@jinshu.view.file.pages', { count: block.pageCount }));
  if (block.sizeBytes !== undefined) metaParts.push(formatBytes(block.sizeBytes));
  if (block.description) metaParts.push(block.description);

  const handleDownload = () => {
    const resolved = safeResolveSrc(block.src, ctx.resolveSrc);
    if (!resolved) return;
    ctx.onFileAction?.({ blockId: block.id, src: resolved, action: 'download' });
  };

  return (
    <view id={blockDomId(block.id, ctx)} data-testid={blockTestId(block.id, ctx)} className={r.file.card}>
      <view className={r.file.main}>
        <text className={r.file.badge}>{fileExtLabel(block)}</text>
        <view className={r.file.info}>
          <text className={r.file.name}>{block.name}</text>
          {metaParts.length > 0 && <text className={r.file.meta}>{metaParts.join(' · ')}</text>}
        </view>
      </view>
      <view className={r.file.actions}>
        {effectiveDisplay === 'inline' && <Badge label={jinshuText('@jinshu.view.file.inline')} tone="info" size="sm" />}
        <Button label={jinshuText('@jinshu.view.file.download')} size="sm" variant="tonal" onClick={handleDownload} />
      </view>
    </view>
  );
}

function JinshuColumnsBlockView({ block, ctx }: { block: ColumnsBlock; ctx: BlockRenderContext }) {
  const { env } = useView();
  // Stack columns vertically on compact screens or enlarged system text scale
  const shouldStack = env.sizeClass === 'compact' || env.textScale >= 1.35;
  return (
    <view
      id={blockDomId(block.id, ctx)}
      data-testid={blockTestId(block.id, ctx)}
      className={shouldStack ? r.columns.stacked : r.columns.base}
    >
      {block.columns.map((col, idx) => {
        const flexGrow = block.widths?.[idx] ?? 1;
        return (
          <view key={col.id} className={r.columns.column} style={shouldStack ? undefined : { flexGrow, flexBasis: 0 }}>
            {col.blocks.map((child, cIdx) => renderBlock(child, cIdx > 0 ? col.blocks[cIdx - 1].type : null, ctx))}
          </view>
        );
      })}
    </view>
  );
}

// A wide character (CJK, fullwidth) is about two `ch`; everything else about one.
const WIDE = /[\u1100-\u115f\u2e80-\ua4cf\uac00-\ud7a3\uf900-\ufaff\ufe30-\ufe4f\uff00-\uff60\uffe0-\uffe6]/u;
const chWidth = (text: string) => {
  let n = 0;
  for (const c of text) n += WIDE.test(c) ? 2 : 1;
  return n;
};
/** The widest piece that cannot break: a word, where a wide character is a piece of its own. */
const longestPiece = (text: string) => {
  let most = 0;
  for (const word of text.split(/\s+/)) {
    if (WIDE.test(word)) most = Math.max(most, 2);
    for (const run of word.split(new RegExp(WIDE.source, 'gu'))) most = Math.max(most, chWidth(run));
  }
  return most;
};

const CELL_PAD_CH = 3;
const MIN_COL_CH = 6;
const MAX_MIN_CH = 16;
const MAX_IDEAL_CH = 48;
const columnSizes = new WeakMap<TableBlock, { readonly min: readonly number[]; readonly ideal: readonly number[] }>();

/**
 * Column sizes from content, in `ch`, as an auto table layout does: `ideal` is a column's longest cell
 * on one line, `min` its widest unbreakable piece. Counted once per table object.
 */
export function tableColumnSizes(block: TableBlock): { readonly min: readonly number[]; readonly ideal: readonly number[] } {
  const hit = columnSizes.get(block);
  if (hit) return hit;
  const n = tableColumnCount(block);
  const min = Array.from({ length: n }, () => MIN_COL_CH);
  const ideal = Array.from({ length: n }, () => MIN_COL_CH);
  for (const row of block.rows) {
    row.cells.forEach((cell, c) => {
      if (c >= n) return;
      const text = plainText(cell);
      ideal[c] = Math.max(ideal[c], Math.min(MAX_IDEAL_CH, chWidth(text)));
      min[c] = Math.max(min[c], Math.min(MAX_MIN_CH, longestPiece(text)));
    });
  }
  const sizes = { min: min.map(m => m + CELL_PAD_CH), ideal: ideal.map((w, c) => Math.max(w, min[c]) + CELL_PAD_CH) };
  columnSizes.set(block, sizes);
  return sizes;
}

/**
 * One column's share of the row, the same in every row: space goes by content (or the table's own
 * `widths`), and no column goes below its widest word, so a cell is never clipped while others have room.
 */
export function tableColumnFlex(block: TableBlock, column: number) {
  const sizes = tableColumnSizes(block);
  return { flexGrow: block.widths?.[column] ?? sizes.ideal[column] ?? 1, flexBasis: 0, flexShrink: 0, minWidth: `${sizes.min[column] ?? MIN_COL_CH}ch` as const };
}

/** The width under which the table scrolls sideways: every column at its minimum. */
export function tableMinWidth(block: TableBlock): `${number}ch` {
  return `${tableColumnSizes(block).min.reduce((a, b) => a + b, 0)}ch`;
}

/** Renders a single `JinshuBlock` with contextual spacing and error isolation. */
export function renderBlock(
  block: JinshuBlock,
  prevType: string | null,
  ctx: BlockRenderContext,
): Child {
  const spacing = blockTopSpacing(prevType, block.type);
  const isHighlighted = ctx.highlightedBlockIds?.has(block.id) === true;
  const wrapClass = cx(spacing, isHighlighted && r.highlightUpdate.base);

  const inlineOpts = {
    blockId: block.id,
    onLinkPress: ctx.onLinkPress,
    onInPageLink: ctx.onInPageLink,
    customInlines: ctx.customInlines,
  };

  let inner: Child;

  switch (block.type) {
    case 'paragraph':
      inner = (
        <text
          id={blockDomId(block.id, ctx)}
          data-testid={blockTestId(block.id, ctx)}
          className={cx(r.paragraph.base, takeSelect(ctx) && r.selectable)}
        >
          {block.runs.length > 0 ? renderInlineRuns(block.runs, inlineOpts) : '\u200B'}
        </text>
      );
      break;

    case 'heading': {
      const headingBlk = block as HeadingBlock;
      inner = (
        <text
          id={blockDomId(headingBlk.id, ctx)}
          data-testid={blockTestId(headingBlk.id, ctx)}
          role="heading"
          headingLevel={headingBlk.level}
          className={cx(r.heading.base, r.heading.level[headingBlk.level], takeSelect(ctx) && r.selectable)}
        >
          {renderInlineRuns(headingBlk.runs, inlineOpts)}
        </text>
      );
      break;
    }

    case 'quote': {
      const q = block as QuoteBlock;
      inner = (
        <view id={blockDomId(q.id, ctx)} data-testid={blockTestId(q.id, ctx)} className={r.quote.base}>
          {q.blocks.map((child, idx) => renderBlock(child, idx > 0 ? q.blocks[idx - 1].type : null, ctx))}
        </view>
      );
      break;
    }

    case 'callout': {
      const c = block as CalloutBlock;
      inner = (
        <view id={blockDomId(c.id, ctx)} data-testid={blockTestId(c.id, ctx)} className={cx(r.callout.base, r.callout.tone[c.tone])}>
          {/* Without a title the tone names itself, so a tint is never the only sign of what it is. */}
          <text className={r.callout.title[c.tone]}>
            {c.title && c.title.length > 0 ? renderInlineRuns(c.title, inlineOpts) : jinshuText('@jinshu.calloutTone', { tone: c.tone })}
          </text>
          <view className={r.callout.body}>
            {c.blocks.map((child, idx) => renderBlock(child, idx > 0 ? c.blocks[idx - 1].type : null, ctx))}
          </view>
        </view>
      );
      break;
    }

    case 'details':
      inner = <JinshuDetailsView block={block as DetailsBlock} ctx={ctx} />;
      break;

    case 'list': {
      const l = block as ListBlock;
      const start = l.start ?? 1;
      inner = (
        <view id={blockDomId(l.id, ctx)} data-testid={blockTestId(l.id, ctx)} role="list" className={r.list.base}>
          {l.items.map((item, idx) => (
            <view key={item.id} role="listitem" className={r.list.item}>
              {l.style === 'task' ? (
                <button
                  role="menuitemcheckbox"
                  aria-checked={item.checked === true}
                  aria-label={plainText(item.runs) || jinshuText('@jinshu.view.task')}
                  disabled={!ctx.onToggleTask}
                  onClick={() => ctx.onToggleTask?.(l.id, item.id, !(item.checked === true))}
                  className={cx(r.list.checkbox, item.checked ? r.list.checkboxDone : r.list.checkboxOpen)}
                >
                  {item.checked && (
                    <vector aria-hidden paths={TICK_PATH} viewBox={[0, 0, 16, 16]} className={r.list.tick} />
                  )}
                </button>
              ) : (
                <text aria-hidden className={r.list.marker}>
                  {l.style === 'ordered' ? `${start + idx}.` : '•'}
                </text>
              )}
              <view className={r.list.body}>
                {item.runs.length > 0 && (
                  <text className={cx(r.paragraph.base, item.checked && r.list.done)}>
                    {renderInlineRuns(item.runs, { ...inlineOpts, blockId: item.id })}
                  </text>
                )}
                {item.blocks && item.blocks.map((child, cIdx) => renderBlock(child, cIdx > 0 ? item.blocks![cIdx - 1].type : null, ctx))}
              </view>
            </view>
          ))}
        </view>
      );
      break;
    }

    case 'code':
      inner = <JinshuCodeBlockView block={block} ctx={ctx} />;
      break;

    case 'math': {
      const m = block as MathBlock;
      inner = (
        <view id={blockDomId(m.id, ctx)} data-testid={blockTestId(m.id, ctx)} aria-label={m.latex || undefined} className={r.math.root}>
          <scroll className={r.math.scroll}>
            <view className={r.math.inner}>
              <MathFormulaView latex={m.latex} />
            </view>
          </scroll>
        </view>
      );
      break;
    }

    case 'table': {
      const t = block as TableBlock;
      inner = (
        <TableFrame table={t} id={blockDomId(t.id, ctx)} testId={blockTestId(t.id, ctx)}>
          {t.rows.map((row, rIdx) => {
            const isHeader = rIdx === 0 && t.headerRow === true;
            return (
              <view key={`${t.id}-${row.id}`} className={isHeader ? r.table.headerRow : r.table.row}>
                {row.cells.map((cell, cIdx) => (
                  <view key={`${row.id}-${cIdx}`} style={tableColumnFlex(t, cIdx)} className={r.table.cell.box}>
                    <text className={isHeader ? r.table.cell.header : r.table.cell.text} style={t.align?.[cIdx] ? { textAlign: t.align[cIdx] } : undefined}>
                      {renderInlineRuns(cell, inlineOpts)}
                    </text>
                  </view>
                ))}
              </view>
            );
          })}
        </TableFrame>
      );
      break;
    }

    case 'columns':
      inner = <JinshuColumnsBlockView block={block} ctx={ctx} />;
      break;

    case 'divider':
      inner = <view id={blockDomId(block.id, ctx)} aria-hidden className={r.divider.base} />;
      break;

    case 'image':
      inner = <JinshuImageBlockView block={block} ctx={ctx} />;
      break;

    case 'video':
      inner = <JinshuVideoBlockView block={block} ctx={ctx} />;
      break;

    case 'audio':
      inner = <JinshuAudioBlockView block={block} ctx={ctx} />;
      break;

    case 'file':
      inner = <JinshuFileBlockView block={block} ctx={ctx} />;
      break;

    default: {
      const ext = block as ExtensionBlock;
      const renderer = ctx.customBlocks?.[ext.type];
      if (renderer) {
        try {
          inner = (
            <view id={blockDomId(ext.id, ctx)} data-testid={blockTestId(ext.id, ctx)}>
              {renderer({
                block: ext as any,
                mode: ctx.mode,
                emit: (type, data) => ctx.onEmitBlockEvent?.(ext.id, type, data),
                onUpdateAttrs: attrs => ctx.onUpdateCustomAttrs?.(ext.id, attrs),
              })}
            </view>
          );
        } catch {
          inner = (
            <view id={blockDomId(ext.id, ctx)} className={r.extension.base}>
              <text className={r.extension.failed}>{jinshuText('@jinshu.view.extensionFailed', { type: ext.type })}</text>
              <text className={r.extension.fallback}>{ext.fallback}</text>
            </view>
          );
        }
      } else {
        inner = (
          <view id={blockDomId(ext.id, ctx)} data-testid={blockTestId(ext.id, ctx)} className={r.extension.base}>
            <text className={r.extension.type}>{ext.type}</text>
            <text className={r.extension.fallback}>{ext.fallback}</text>
          </view>
        );
      }
      break;
    }
  }

  return (
    <view key={block.id} className={wrapClass}>
      {inner}
    </view>
  );
}

/** A document still loading: the reader and the editor show it until the first snapshot. */
export function JinshuSkeleton() {
  return (
    <SkeletonGroup label={jinshuText('@jinshu.editor.loading')} className={r.skeleton.base}>
      <Skeleton shape="text" className={r.skeleton.title} />
      <Skeleton shape="text" lines={3} className={r.skeleton.lines} />
      <Skeleton shape="block" className={r.skeleton.block} />
    </SkeletonGroup>
  );
}

/**
 * One top-level block as its own component, so a render that changed nothing it shows keeps it.
 * `ctx` must hold no per-render objects: the budget and highlight arrive as plain props.
 */
export function BlockBody({
  block,
  prevType = null,
  ctx,
  selectLeft,
  isHighlighted = false,
}: {
  readonly block: JinshuBlock;
  readonly prevType?: string | null;
  readonly ctx: BlockRenderContext;
  readonly selectLeft?: number;
  readonly isHighlighted?: boolean;
}) {
  const own: BlockRenderContext = {
    ...ctx,
    ...(selectLeft !== undefined ? { selectBudget: { left: selectLeft } } : {}),
    ...(isHighlighted ? { highlightedBlockIds: new Set([block.id]) } : {}),
  };
  return <>{renderBlock(block, prevType, own)}</>;
}
