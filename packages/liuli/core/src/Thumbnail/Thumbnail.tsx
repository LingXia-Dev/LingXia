import { cx } from '@lingxia/view';
import { CircularProgress } from '../CircularProgress/CircularProgress';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { thumbnailRecipe as r } from './Thumbnail.recipe';
import { dataOf } from '../internal/data';

export type ThumbnailSize = 'sm' | 'md' | 'lg' | 'fill';
export type ThumbnailRatio = 'square' | 'landscape' | 'wide';

export interface ThumbnailProps {
  src: string;
  /** What the picture shows. With `onClick` it is the button's name. */
  alt: string;
  /** 48, 72 or 96 px wide; `fill` takes the width it is given, as in a grid. @default 'md' */
  size?: ThumbnailSize;
  /** 1:1, 4:3 or 16:9; the picture is cropped to fill it. @default 'square' */
  ratio?: ThumbnailRatio;
  /** Makes it a button: opens the picture, usually in a Lightbox. */
  onClick?: () => void;
  /** Marks it as the one in view among several. @default false */
  isSelected?: boolean;
  /** Adds a remove button in its top corner, as for an attachment that can be taken back. */
  onRemove?: () => void;
  /** The remove button's name. Defaults to "Remove <alt>" in the page's language. */
  removeLabel?: string;
  /** Covers it with a spinner while what it stands for is still on its way, such as an upload. @default false */
  isLoading?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** A small picture that stands for a bigger one. */
export function Thumbnail({ src, alt, size = 'md', ratio = 'square', onClick, isSelected = false, onRemove, removeLabel, isLoading = false, className, 'data-testid': testId, ...attributes }: ThumbnailProps) {
  const classes = cx(r.root.base, r.root.size[size], r.root.ratio[ratio], isSelected && r.root.selected);
  const loading = isLoading && (
    <view aria-hidden className={r.loading.base}>
      <CircularProgress label={liuliText('@liuli.loadingNamed', { name: alt })} size="sm" className={r.loading.ring} />
    </view>
  );
  const picture = onClick === undefined ? (
    <view data-testid={onRemove === undefined ? testId : undefined} aria-busy={isLoading || undefined} {...(onRemove === undefined ? dataOf(attributes) : {})} className={cx(classes, onRemove === undefined && className)}>
      <image src={src} alt={alt} className={r.image.base} />
      {loading}
    </view>
  ) : (
    <button aria-label={alt} aria-selected={isSelected || undefined} aria-busy={isLoading || undefined} onClick={onClick} data-testid={onRemove === undefined ? testId : undefined} {...(onRemove === undefined ? dataOf(attributes) : {})} className={cx(classes, r.root.pressable, onRemove === undefined && className)}>
      <image src={src} alt="" className={r.image.base} />
      {loading}
    </button>
  );
  if (onRemove === undefined) return picture;
  // The remove button sits beside the picture, never inside a picture that is itself a button.
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.frame.base, r.frame.size[size], className)}>
      {picture}
      <button aria-label={removeLabel ?? liuliText('@liuli.removeNamed', { name: alt })} onClick={onRemove} data-testid={testId && `${testId}-remove`} className={r.remove.base}>
        <view className={r.remove.disc}>
          <Glyph name="close" weight={2.5} side={12} className={r.remove.icon} />
        </view>
      </button>
    </view>
  );
}
