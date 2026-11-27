import {
  cx,
  reveal,
  useChannel,
  useId,
  useLocal,
  type Child,
  type Video,
} from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { IconButton } from '@liulidesign/core/IconButton';
import { Lightbox, type LightboxImage } from '@liulidesign/core/Lightbox';
import { Tooltip } from '@liulidesign/core/Tooltip';
import type { AgentActivity, AgentBatch } from '../source/types';
import { blockContentHash, findBlockById, listBlocks, plainText, walkBlocks } from '../schema/helpers';
import { validateSrc } from '../schema/validate';
import type {
  DistributiveOmit,
  FilePolicy,
  HistoryEntry,
  JinshuDocument,
  JinshuProgress,
  MediaPolicy,
  Observation,
} from '../schema/types';
import type { InlineAtomRenderer } from '../internal/InlineRuns';
import { blockElementId, scrollToInPage, setNavReader, type JinshuNavHandle } from '../internal/nav';
import { newReadingState, readVisibility, type ReadingState } from '../internal/reading';
import { useJinshuChannelContext, type JinshuChannelAction } from '../internal/channel';
import { jinshuText } from '../i18n/text';
import { BlockBody, JinshuSkeleton, type BlockRenderContext, type CustomBlockRenderer, type ResumeMedia } from '../internal/Blocks';
import { selectCost } from '../internal/select';
import { stableCallbacks } from '../internal/stable';
import { jinshuViewRecipe as r } from './JinshuView.recipe';

export { JinshuProvider, type JinshuChannelAction, type JinshuProviderProps } from '../internal/channel';
export type { CustomBlockRenderer } from '../internal/Blocks';
export type { InlineAtomRenderer } from '../internal/InlineRuns';
export type { JinshuNavHandle } from '../internal/nav';

export interface JinshuViewProps {
  /** The document, held by the page: `null` says it is unavailable, `undefined` that it is still loading. Leave it out to read a live `channel`. */
  readonly doc?: JinshuDocument | null;
  /** The page's `syncDoc` action, which Logic answers with `source.serve`: the reader follows the live document. */
  readonly channel?: JinshuChannelAction;
  /** Which document `channel` serves. */
  readonly docKey?: string;
  /** From `useJinshuNav()`, shared with a `JinshuOutline`: pressing a heading scrolls the reader to it. Give `nav.onVisibleChange` to the `<scroll>` holding the reader to follow the heading being read and report dwell and position. */
  readonly nav?: JinshuNavHandle;
  /** Saved progress: offers to continue at its position and resumes media where it stopped. */
  readonly resume?: JinshuProgress;
  /** A top-level block to open at, once: where an agent's change landed (a tool result's `page`). */
  readonly openAt?: string;
  /** What was read and played, in batches: played media ranges and events from custom blocks; dwell per block and the reading position once `nav` hears the scroll. Facts only; the app decides what counts as done. */
  readonly onObserve?: (events: readonly Observation[]) => void;
  /** Whether files may show inline or only as attachments. */
  readonly filePolicy?: FilePolicy;
  /** How media may be scrubbed (`any`, `played-only`, `none`) and the fastest rate offered. */
  readonly mediaPolicy?: MediaPolicy;
  /** Turns a block's `src` into what the page loads: a signed URL, a custom scheme. */
  readonly resolveSrc?: (src: string) => string;
  /** Renderers for the app's own block types, keyed by `x-scope-name`. */
  readonly customBlocks?: Readonly<Record<string, CustomBlockRenderer>>;
  /** Renderers for the app's own inline atoms, keyed by `x-scope-name`. */
  readonly customInlines?: Readonly<Record<string, InlineAtomRenderer>>;
  /** Tints the blocks a live update changed, briefly. */
  readonly hasUpdateHighlights?: boolean;
  /** On a live `channel`, shows what agents do: a mark where one is writing, and a bar naming its latest change, with View and Dismiss, over the blocks it marks. @default true */
  readonly hasAgentFeedback?: boolean;
  /** A link to outside the document was pressed; in-page links scroll on their own. */
  readonly onLinkPress?: (info: { readonly blockId: string; readonly href: string }) => void;
  /** A file's Download was pressed; without it, the button does nothing. */
  readonly onFileAction?: (info: { readonly blockId: string; readonly src: string; readonly action: 'download' }) => void;
  /** Puts the code on the clipboard; a code block offers Copy only when set. */
  readonly onCopyCode?: (code: string, blockId: string) => void;
  /** Text can be selected and copied, up to the page's budget of selectable nodes. @default true */
  readonly isSelectable?: boolean;
  /** Shows the document's title above its blocks. @default true */
  readonly hasTitle?: boolean;
  /** Fills its container instead of a 65-character reading measure. @default false */
  readonly isFullWidth?: boolean;
  /** Drawn in place of the document when it is unavailable. Defaults to one line in the page's language. */
  readonly fallback?: Child;
  readonly className?: string;
  readonly 'data-testid'?: string;
}


// What a reader keeps outside render state, for as long as `anchor` (one of its cells) lives.
const lives = new WeakMap<object, { readonly reading: ReadingState; readonly players: Map<string, Video> }>();
function liveOf(anchor: object) {
  let live = lives.get(anchor);
  if (!live) lives.set(anchor, (live = { reading: newReadingState(), players: new Map() }));
  return live;
}

/** What agents are doing, as a live reader hears it. */
interface AgentFeedback {
  readonly activities: readonly AgentActivity[];
  /** Not yet dismissed or undone, oldest first. */
  readonly batches: readonly AgentBatch[];
  readonly dismiss: (batchId: string) => void;
}

function LiveJinshuView(props: JinshuViewProps & { readonly activeChannel: JinshuChannelAction; readonly docKey: string }) {
  // Until the first snapshot the document is loading, not missing: the skeleton shows, not "unavailable".
  const snapshot = useLocal<{
    doc?: JinshuDocument;
    filePolicy?: FilePolicy;
    highlightedIds: string[];
    activities: AgentActivity[];
    batches: AgentBatch[];
  }>(`jinshu-live-${props.docKey}`, { highlightedIds: [], activities: [], batches: [] });

  const channel = useChannel(`jinshu-view-${props.docKey}`, props.activeChannel, {
    params: { doc: props.docKey, role: 'reader' },
    onData(msg) {
      const cur = snapshot.value;
      if (msg.t === 'snapshot') {
        const nextDoc = msg.doc as JinshuDocument;
        const prevDoc = cur.doc;
        const changed: string[] = [];
        if (props.hasUpdateHighlights && prevDoc) {
          const prevHashes = new Map(prevDoc.blocks.map(b => [b.id, blockContentHash(b)]));
          for (const b of nextDoc.blocks) {
            if (prevHashes.get(b.id) !== blockContentHash(b)) changed.push(b.id);
          }
        }
        snapshot.set({ ...cur, doc: nextDoc, filePolicy: msg.policy?.filePolicy, highlightedIds: changed });
      } else if (msg.t === 'activity') {
        snapshot.set({ ...cur, activities: msg.activities as AgentActivity[] });
      } else if (msg.t === 'agentBatches') {
        snapshot.set({ ...cur, batches: [...cur.batches, ...(msg.batches as AgentBatch[])] });
      } else if (msg.t === 'history' && cur.batches.length > 0) {
        // A batch undone from anywhere drops its bar and marks.
        const kept = new Set((msg.entries as HistoryEntry[]).filter(h => !h.undone).map(h => h.batchId));
        const batches = cur.batches.filter(b => kept.has(b.batchId));
        if (batches.length !== cur.batches.length) snapshot.set({ ...cur, batches });
      }
    },
  });

  const agent: AgentFeedback | undefined =
    props.hasAgentFeedback === false
      ? undefined
      : {
          activities: snapshot.value.activities,
          batches: snapshot.value.batches,
          dismiss: batchId => snapshot.set({ ...snapshot.value, batches: snapshot.value.batches.filter(b => b.batchId !== batchId) }),
        };

  return (
    <ControlledJinshuView
      {...props}
      doc={snapshot.value.doc ?? (channel.error ? null : undefined)}
      filePolicy={props.filePolicy ?? snapshot.value.filePolicy}
      highlightedIds={snapshot.value.highlightedIds}
      agent={agent}
    />
  );
}

/** The top-level block that holds `id`, for marks and jumps on nested changes; null once deleted. */
function topLevelOf(id: string, doc: JinshuDocument): string | null {
  if (doc.blocks.some(b => b.id === id)) return id;
  return doc.blocks.find(b => JSON.stringify(b).includes(`"id":"${id}"`))?.id ?? null;
}

/** Changed top-level blocks in document order. */
function changedTopIds(batches: readonly AgentBatch[], doc: JinshuDocument): string[] {
  const ids = new Set<string>();
  for (const batch of batches) {
    for (const id of batch.blockIds) {
      const top = topLevelOf(id, doc);
      if (top) ids.add(top);
    }
  }
  return doc.blocks.filter(b => ids.has(b.id)).map(b => b.id);
}

function ControlledJinshuView({
  doc,
  nav,
  resume,
  openAt,
  onObserve,
  filePolicy,
  mediaPolicy,
  resolveSrc,
  customBlocks,
  customInlines,
  highlightedIds,
  agent,
  onLinkPress,
  onFileAction,
  onCopyCode,
  isSelectable = true,
  hasTitle = true,
  isFullWidth = false,
  fallback,
  className,
  'data-testid': testId,
}: JinshuViewProps & { readonly highlightedIds?: readonly string[]; readonly agent?: AgentFeedback }) {
  const ownScope = useId();
  // Driven by a nav, the view takes its scope so the nav's `scrollTo` finds the blocks.
  const scope = nav ? nav.scope : ownScope;
  const obsSeq = useLocal('jinshu-obs-seq', 0);
  const obsSession = useLocal('jinshu-obs-session', '');
  const lightboxOpen = useLocal('jinshu-lb-open', false);
  const lightboxIndex = useLocal('jinshu-lb-idx', 0);
  const resumeDismissed = useLocal('jinshu-resume-dismissed', false);
  const openedAt = useLocal('jinshu-opened-at', '');
  // Without a nav, an in-page link's target, so a fold holding it opens.
  const ownTarget = useLocal<string | null>('jinshu-link-target', null);
  // Where View is in the latest batch's changed blocks.
  const changeCursor = useLocal('jinshu-change-cursor', { batch: '', at: 0 });
  const live = liveOf(obsSeq);

  if (doc === undefined) {
    return (
      <view data-testid={testId} className={cx(isFullWidth ? r.root.wide : r.root.base, className)}>
        <JinshuSkeleton />
      </view>
    );
  }

  if (doc === null) {
    return (
      <view data-testid={testId} className={cx(isFullWidth ? r.root.wide : r.root.base, className)}>
        {fallback ?? <text className={r.unavailable.base}>{jinshuText('@jinshu.view.unavailable')}</text>}
      </view>
    );
  }

  const highlightedSet = highlightedIds && highlightedIds.length > 0 ? new Set(highlightedIds) : undefined;

  // `at` is the event's time, when it has one.
  const emitObservations = (events: readonly DistributiveOmit<Observation, 'seq' | 'sessionId'>[], at?: number) => {
    if (!onObserve || events.length === 0) return;
    // One session per mount: a reopened page restarts `seq`, which `reduceObservations` must not drop.
    // Named by the first event's time, else by how many sessions `resume` has seen.
    let sessionId = obsSession.value;
    if (!sessionId) {
      sessionId = `${scope}-${at !== undefined ? Math.round(at).toString(36) : `r${Object.keys(resume?.seenSeq ?? {}).length}`}`;
      obsSession.set(sessionId);
    }
    const first = obsSeq.value + 1;
    obsSeq.set(first + events.length - 1);
    onObserve(events.map((event, idx) => ({ ...event, seq: first + idx, sessionId }) as Observation));
  };
  const emitObservation = (event: DistributiveOmit<Observation, 'seq' | 'sessionId'>, at?: number) => emitObservations([event], at);

  // The View caps selectable text nodes per page: the first 56 text nodes stay selectable,
  // the rest render as plain text instead of switching selection off for the whole document.
  let selectLeft = 56;

  // Resume : offer to jump back to the last reading position; media seeks on its own metadata.
  const resumeBlockId = resume?.position?.blockId;
  const resumeTarget = resumeBlockId ? findBlockById(doc, resumeBlockId) : undefined;
  const resumeLabel =
    resumeTarget && (resumeTarget.type === 'heading' || resumeTarget.type === 'paragraph') ? plainText(resumeTarget.runs).slice(0, 24) : '';
  const resumeMedia: Record<string, ResumeMedia> = {};
  for (const [id, m] of Object.entries(resume?.media ?? {})) {
    resumeMedia[id] = { src: m.src, positionMs: m.positionMs, playedMs: m.played.reduce((most, [, end]) => Math.max(most, end), 0) };
  }

  const target = nav ? nav.state.targetBlockId : ownTarget.value;
  const scrollToTarget = (id: string) => {
    const resolved = scrollToInPage(doc, id, scope, nav);
    if (resolved && !nav) ownTarget.set(resolved);
  };

  // Agent feedback, read-only: every batch not dismissed or undone marks its blocks; the bar is the latest
  // batch, and dismissing it shows the one before, as in the editor.
  const batches = agent?.batches ?? [];
  const changedSet = batches.length > 0 ? new Set(changedTopIds(batches, doc)) : undefined;
  const latestBatch = batches[batches.length - 1];
  const latestOrder = latestBatch ? changedTopIds([latestBatch], doc) : [];
  // What View steps through, plus blocks the batch deleted.
  const latestCount = latestOrder.length + (latestBatch ? latestBatch.blockIds.filter(id => topLevelOf(id, doc) === null).length : 0);
  const nextChange = () => {
    if (!latestBatch || latestOrder.length === 0) return;
    const at = changeCursor.value.batch === latestBatch.batchId ? changeCursor.value.at % latestOrder.length : 0;
    changeCursor.set({ batch: latestBatch.batchId, at: at + 1 });
    scrollToTarget(latestOrder[at]);
  };
  // Only writing: a reader is told the text is changing, not who is looking at it.
  const markByBlock = new Map<string, AgentActivity>();
  const topMarks: AgentActivity[] = [];
  for (const act of agent?.activities ?? []) {
    if (act.kind !== 'writing') continue;
    const top = act.blockIds.length > 0 ? topLevelOf(act.blockIds[0], doc) : null;
    if (top && !markByBlock.has(top)) markByBlock.set(top, act);
    else if (!top) topMarks.push(act);
  }
  const renderActivity = (act: AgentActivity, onBlock: boolean) => (
    <view key={`act-${act.id}`} role="status" className={cx(r.activity.base, onBlock && r.activity.block)}>
      <view aria-hidden className={r.activity.dot} />
      <text className={r.activity.text}>{jinshuText('@jinshu.view.agent.writing', { name: act.origin.label ?? act.origin.id })}</text>
    </view>
  );

  // The page's scroll reports what it shows through the nav; the reader turns that into facts.
  if (nav) {
    const topIds = new Set(doc.blocks.map(b => b.id));
    const prefix = blockElementId(scope, '');
    setNavReader(nav, event => {
      const shown = event.items.flatMap(item => {
        const id = item.id.startsWith(prefix) ? item.id.slice(prefix.length) : '';
        return topIds.has(id) ? [{ id, ratio: item.ratio, top: item.top }] : [];
      });
      const { facts, left, headingId } = readVisibility(live.reading, doc, event.timeStamp, shown);
      for (const id of left) {
        const block = doc.blocks.find(b => b.id === id);
        if (!block) continue;
        walkBlocks([block], b => {
          const player = (b.type === 'video' || b.type === 'audio') && live.players.get(b.id);
          if (player && !player.paused) void player.pause();
        });
      }
      if (headingId !== undefined) nav.setActiveHeading(headingId);
      emitObservations(facts.map(fact => {
        if (fact.kind !== 'dwell') return fact;
        const block = doc.blocks.find(b => b.id === fact.blockId);
        return block ? { ...fact, contentHash: blockContentHash(block) } : fact;
      }), event.timeStamp);
    });
  }

  // Collect all valid image blocks in document order for seamless multi-image Lightbox browsing.
  const galleryImages: { blockId: string; item: LightboxImage }[] = [];
  for (const img of listBlocks(doc, 'image')) {
    const resolved = resolveSrc ? resolveSrc(img.src) : img.src;
    if (validateSrc(resolved)) continue;
    galleryImages.push({
      blockId: img.id,
      item: {
        src: resolved,
        alt: img.alt ?? '',
        ...(img.caption && img.caption.length > 0 ? { caption: plainText(img.caption) } : {}),
      },
    });
  }

  // Each block is its own component: its callbacks keep their identity, so a parent's re-render
  // that changed nothing a block shows does not draw it again.
  const ctx: BlockRenderContext = {
    mode: 'view',
    idScope: scope,
    testScope: testId,
    filePolicy,
    mediaPolicy,
    customBlocks,
    customInlines,
    isSelectable,
    resumeMedia,
    players: live.players,
    targetBlockId: target,
    ...stableCallbacks(lightboxOpen, {
      resolveSrc,
      onLinkPress,
      onInPageLink: scrollToTarget,
      onFileAction,
      onOpenImage: (blockId: string) => {
        const idx = galleryImages.findIndex(g => g.blockId === blockId);
        if (idx >= 0) {
          lightboxIndex.set(idx);
          lightboxOpen.set(true);
        }
      },
      onCopyCode,
      onEmitBlockEvent: (blockId: string, type: string, data: unknown) => {
        emitObservation({ kind: 'block', blockId, type, data });
      },
      onPlayedSegment: (blockId: string, src: string, range: [number, number], durationMs: number, rate: number, ended?: true, at?: number) => {
        emitObservation({
          kind: 'played',
          blockId,
          src,
          range,
          durationMs,
          rate,
          ...(ended ? { ended: true } : {}),
        }, at);
      },
    }),
  };

  return (
    <view data-testid={testId} className={cx(isFullWidth ? r.root.wide : r.root.base, className)}>
      {hasTitle && doc.title && (
        <text role="heading" headingLevel={1} className={r.docTitle.base}>
          {doc.title}
        </text>
      )}
      {(latestBatch || topMarks.length > 0) && (
        <view className={r.agentHeader.base}>
          {latestBatch && (
            <view role="status" className={r.agentBar.base} data-testid={testId && `${testId}-agent-bar`}>
              <text className={r.agentBar.text}>
                {jinshuText(latestBatch.away ? '@jinshu.view.agent.changedAway' : '@jinshu.view.agent.changed', {
                  name: latestBatch.origin.label ?? latestBatch.origin.id,
                  count: latestCount,
                })}
              </text>
              <view className={r.agentBar.actions}>
                {latestOrder.length > 0 && (
                  <Button label={jinshuText('@jinshu.view.agent.view')} size="sm" variant="ghost" onClick={nextChange} />
                )}
                <Tooltip content={jinshuText('@jinshu.view.agent.dismiss')} placement="bottom">
                  <IconButton label={jinshuText('@jinshu.view.agent.dismiss')} icon="close" size="sm" onClick={() => agent?.dismiss(latestBatch.batchId)} />
                </Tooltip>
              </view>
            </view>
          )}
          {topMarks.map(act => renderActivity(act, false))}
        </view>
      )}
      {resumeTarget && !resumeDismissed.value && (
        <view className={r.resume.base}>
          <text className={r.resumeText.base}>
            {resumeLabel ? jinshuText('@jinshu.view.resumeAt', { heading: resumeLabel }) : jinshuText('@jinshu.view.resumeHere')}
          </text>
          <Button
            label={jinshuText('@jinshu.view.resume')}
            size="sm"
            variant="ghost"
            data-testid={testId && `${testId}-resume`}
            onClick={() => {
              resumeDismissed.set(true);
              scrollToTarget(resumeTarget.id);
              emitObservation({ kind: 'position', blockId: resumeTarget.id, offset: resume?.position?.offset ?? 0 });
            }}
          />
          <IconButton label={jinshuText('@jinshu.view.fromStart')} icon="close" size="sm" onClick={() => resumeDismissed.set(true)} />
        </view>
      )}
      {doc.blocks.flatMap((block, idx) => {
        const allowed = isSelectable ? selectLeft : 0;
        selectLeft = Math.max(0, selectLeft - selectCost(block));
        // A sibling keyed apart, so the block stays mounted as the mark comes and goes.
        const mark = markByBlock.get(block.id);
        const before = mark ? [renderActivity(mark, true)] : [];
        const node = (
          <BlockBody
            key={block.id}
            block={block}
            prevType={idx > 0 ? doc.blocks[idx - 1].type : null}
            ctx={ctx}
            selectLeft={allowed}
            isHighlighted={highlightedSet?.has(block.id) === true}
            isAgentChanged={changedSet?.has(block.id) === true}
          />
        );
        if (block.id !== openAt) return [...before, node];
        // Scrolls there on its first layout; the flag keeps a later relayout from pulling the reader back.
        return [
          ...before,
          <view key={block.id} id={`jinshu-${scope}-o-${block.id}`} onLayout={() => {
            if (openedAt.value === block.id) return;
            openedAt.set(block.id);
            reveal(`jinshu-${scope}-o-${block.id}`);
          }}>
            {node}
          </view>,
        ];
      })}
      {galleryImages.length > 0 && (
        <Lightbox
          isOpen={lightboxOpen.value}
          onOpenChange={open => lightboxOpen.set(open)}
          index={lightboxIndex.value}
          onIndexChange={idx => lightboxIndex.set(idx)}
          label={doc.title ? jinshuText('@jinshu.view.imagesOf', { title: doc.title }) : jinshuText('@jinshu.view.images')}
          images={galleryImages.map(g => g.item)}
        />
      )}
    </view>
  );
}

/**
 * Publication-grade GPU View renderer for Jinshu documents .
 * Minimal usage: `<JinshuView doc={doc} />`, or live channel sync: `<JinshuView channel={actions.syncDoc} docKey="lesson-42" />`.
 */
export function JinshuView(props: JinshuViewProps) {
  const ctxChannel = useJinshuChannelContext();
  const activeChannel = props.channel ?? ctxChannel;
  if (activeChannel && props.docKey && props.doc === undefined) {
    return <LiveJinshuView {...props} activeChannel={activeChannel} docKey={props.docKey} />;
  }
  return <ControlledJinshuView {...props} />;
}
