import {
  cx,
  reveal,
  useChannel,
  useId,
  useLocal,
  type Child,
} from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { IconButton } from '@liulidesign/core/IconButton';
import { Lightbox, type LightboxImage } from '@liulidesign/core/Lightbox';
import { blockContentHash, findBlockById, listBlocks, plainText } from '../schema/helpers';
import { validateSrc } from '../schema/validate';
import type {
  DistributiveOmit,
  FilePolicy,
  JinshuDocument,
  JinshuProgress,
  MediaPolicy,
  Observation,
} from '../schema/types';
import type { InlineAtomRenderer } from '../internal/InlineRuns';
import { blockElementId, type JinshuNavHandle } from '../internal/nav';
import { useJinshuChannelContext, type JinshuChannelAction } from '../internal/channel';
import { jinshuText } from '../i18n/text';
import { BlockBody, JinshuSkeleton, type BlockRenderContext, type CustomBlockRenderer } from '../internal/Blocks';
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
  /** Shared with a `JinshuOutline` from `useJinshuNav()`: pressing a heading in the outline scrolls the reader to it. */
  readonly nav?: JinshuNavHandle;
  /** Saved progress: offers to continue at its position and resumes media where it stopped. */
  readonly resume?: JinshuProgress;
  /** A top-level block to open at, once: where an agent's change landed (a tool result's `page`). */
  readonly openAt?: string;
  /** What was read and played, in batches: dwell per block, played media ranges, the reading position, events from custom blocks. Facts only; the app decides what counts as done. */
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


function LiveJinshuView(props: JinshuViewProps & { readonly activeChannel: JinshuChannelAction; readonly docKey: string }) {
  // Until the first snapshot the document is loading, not missing: the skeleton shows, not "unavailable".
  const snapshot = useLocal<any>(`jinshu-live-${props.docKey}`, {
    doc: undefined as JinshuDocument | undefined,
    filePolicy: undefined as FilePolicy | undefined,
    highlightedIds: [] as string[],
  });

  const channel = useChannel(`jinshu-view-${props.docKey}`, props.activeChannel, {
    params: { doc: props.docKey, role: 'reader' },
    onData(msg) {
      if (msg.t === 'snapshot') {
        const nextDoc = msg.doc as JinshuDocument;
        const prevDoc = snapshot.value.doc as JinshuDocument | undefined;
        const changed: string[] = [];
        if (props.hasUpdateHighlights && prevDoc) {
          const prevHashes = new Map(prevDoc.blocks.map(b => [b.id, blockContentHash(b)]));
          for (const b of nextDoc.blocks) {
            if (prevHashes.get(b.id) !== blockContentHash(b)) changed.push(b.id);
          }
        }
        snapshot.set({
          doc: nextDoc,
          filePolicy: msg.policy?.filePolicy,
          highlightedIds: changed,
        });
      }
    },
  });

  return (
    <ControlledJinshuView
      {...props}
      doc={snapshot.value.doc ?? (channel.error ? null : undefined)}
      filePolicy={props.filePolicy ?? snapshot.value.filePolicy}
      highlightedIds={snapshot.value.highlightedIds}
    />
  );
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
  onLinkPress,
  onFileAction,
  onCopyCode,
  isSelectable = true,
  hasTitle = true,
  isFullWidth = false,
  fallback,
  className,
  'data-testid': testId,
}: JinshuViewProps & { readonly highlightedIds?: readonly string[] }) {
  const ownScope = useId();
  // Driven by a nav, the view takes its scope so the nav's `scrollTo` finds the blocks.
  const scope = nav ? nav.scope : ownScope;
  const obsSeq = useLocal('jinshu-obs-seq', 0);
  const lightboxOpen = useLocal('jinshu-lb-open', false);
  const lightboxIndex = useLocal('jinshu-lb-idx', 0);
  const resumeDismissed = useLocal('jinshu-resume-dismissed', false);
  const openedAt = useLocal('jinshu-opened-at', '');

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

  const emitObservation = (event: DistributiveOmit<Observation, 'seq' | 'sessionId'>) => {
    if (!onObserve) return;
    const nextSeq = obsSeq.value + 1;
    obsSeq.set(nextSeq);
    onObserve([
      {
        ...event,
        seq: nextSeq,
        sessionId: 'view',
      } as Observation,
    ]);
  };

  // The View caps selectable text nodes per page: the first 56 text nodes stay selectable,
  // the rest render as plain text instead of switching selection off for the whole document.
  let selectLeft = 56;

  // Resume : offer to jump back to the last reading position; media seeks on its own metadata.
  const resumeBlockId = resume?.position?.blockId;
  const resumeTarget = resumeBlockId ? findBlockById(doc, resumeBlockId) : undefined;
  const resumeLabel =
    resumeTarget && (resumeTarget.type === 'heading' || resumeTarget.type === 'paragraph') ? plainText(resumeTarget.runs).slice(0, 24) : '';
  const resumeMedia: Record<string, { positionMs: number }> = {};
  for (const [id, m] of Object.entries(resume?.media ?? {})) resumeMedia[id] = { positionMs: m.positionMs };

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
    targetBlockId: nav?.state.targetBlockId,
    ...stableCallbacks(lightboxOpen, {
      resolveSrc,
      onLinkPress,
      onInPageLink: (targetId: string) => nav?.scrollTo(targetId),
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
      onPlayedSegment: (blockId: string, src: string, range: [number, number], durationMs: number, rate: number, ended?: true) => {
        emitObservation({
          kind: 'played',
          blockId,
          src,
          range,
          durationMs,
          rate,
          ...(ended ? { ended: true } : {}),
        });
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
              if (nav) nav.scrollTo(resumeTarget.id);
              else reveal(blockElementId(scope, resumeTarget.id));
              emitObservation({ kind: 'position', blockId: resumeTarget.id, offset: resume?.position?.offset ?? 0 });
            }}
          />
          <IconButton label={jinshuText('@jinshu.view.fromStart')} icon="close" size="sm" onClick={() => resumeDismissed.set(true)} />
        </view>
      )}
      {doc.blocks.map((block, idx) => {
        const allowed = isSelectable ? selectLeft : 0;
        selectLeft = Math.max(0, selectLeft - selectCost(block));
        const node = (
          <BlockBody
            key={block.id}
            block={block}
            prevType={idx > 0 ? doc.blocks[idx - 1].type : null}
            ctx={ctx}
            selectLeft={allowed}
            isHighlighted={highlightedSet?.has(block.id) === true}
          />
        );
        if (block.id !== openAt) return node;
        // Scrolls there on its first layout; the flag keeps a later relayout from pulling the reader back.
        return (
          <view key={block.id} id={`jinshu-${scope}-o-${block.id}`} onLayout={() => {
            if (openedAt.value === block.id) return;
            openedAt.set(block.id);
            reveal(`jinshu-${scope}-o-${block.id}`);
          }}>
            {node}
          </view>
        );
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
