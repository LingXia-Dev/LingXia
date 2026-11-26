import { cx, type Child } from '@lingxia/view';
import { skeletonRecipe as r } from './Skeleton.recipe';
import { dataOf } from '../internal/data';

export type SkeletonShape = 'text' | 'block' | 'circle';
/** The type a text placeholder stands in for. */
export type SkeletonTextSize = 'caption' | 'footnote' | 'body' | 'callout' | 'title' | 'display';

export interface SkeletonProps {
  /** `text` draws lines as tall as the text they stand in for; `block` and `circle` take their size from `className`. @default 'text' */
  shape?: SkeletonShape;
  /** With `text`, the type it stands in for: each line is that type's line, and grows with the reader's text size. @default 'body' */
  textSize?: SkeletonTextSize;
  /** With `text`, how many lines; the last is shorter, as a paragraph's is. @default 1 */
  lines?: number;
  /** Size and placement: `h-32`, `size-10`, `w-2/3`. */
  className?: string;
  'data-testid'?: string;
}

/**
 * A quiet stand-in for content that is on its way. It is hidden from assistive tech; the
 * SkeletonGroup around it says what is loading. A soft glint crosses it, and stops under reduced motion.
 */
export function Skeleton({ shape = 'text', textSize = 'body', lines = 1, className, 'data-testid': testId, ...attributes }: SkeletonProps) {
  if (shape !== 'text') {
    return (
      <view aria-hidden data-testid={testId} {...dataOf(attributes)} className={cx(r.bone.base, r.bone.shape[shape], className)}>
        <view className={r.glint.base} />
      </view>
    );
  }
  const count = Math.max(1, Math.floor(lines));
  return (
    <view aria-hidden data-testid={testId} {...dataOf(attributes)} className={cx(r.lines.base, className)}>
      {Array.from({ length: count }, (_, index) => (
        <view key={String(index)} className={cx(r.line.base, count > 1 && index === count - 1 && r.line.last)}>
          {/* An empty run in the type gives the line its height; the bar is that type's x-height band, centred. */}
          <text className={cx(r.ghost.base, r.size[textSize])}>{'\u200B'}</text>
          <view className={cx(r.bone.base, r.bone.shape.text, r.size[textSize])}>
            <view className={r.glint.base} />
          </view>
        </view>
      ))}
    </view>
  );
}

export interface SkeletonGroupProps {
  /** What is loading, read once by assistive tech: "Loading notes". */
  label: string;
  /** Skeletons laid out as the content will be. */
  children: Child;
  className?: string;
  'data-testid'?: string;
}

/** Skeletons that stand for one piece of content, reported as busy and named by what is loading. */
export function SkeletonGroup({ label, children, className, 'data-testid': testId, ...attributes }: SkeletonGroupProps) {
  return (
    <view role="status" aria-label={label} aria-busy data-testid={testId} {...dataOf(attributes)} className={cx(r.group.base, className)}>
      {children}
    </view>
  );
}
