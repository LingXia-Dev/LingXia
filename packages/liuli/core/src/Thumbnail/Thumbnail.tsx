import { cx } from '@lingxia/view';
import { thumbnailRecipe as r } from './Thumbnail.recipe';

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
  className?: string;
  'data-testid'?: string;
}

/** A small picture that stands for a bigger one. */
export function Thumbnail({ src, alt, size = 'md', ratio = 'square', onClick, isSelected = false, className, 'data-testid': testId }: ThumbnailProps) {
  const classes = cx(r.root.base, r.root.size[size], r.root.ratio[ratio], isSelected && r.root.selected, className);
  if (onClick === undefined) {
    return (
      <view data-testid={testId} className={classes}>
        <image src={src} alt={alt} className={r.image.base} />
      </view>
    );
  }
  return (
    <button aria-label={alt} aria-selected={isSelected || undefined} onClick={onClick} data-testid={testId} className={cx(classes, r.root.pressable)}>
      <image src={src} alt="" className={r.image.base} />
    </button>
  );
}
