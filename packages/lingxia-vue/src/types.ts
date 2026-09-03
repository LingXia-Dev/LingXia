import type { CSSProperties } from 'vue';
import type {
  LxMediaSwiperAttributes,
  LxNavigatorEvent,
  LxVideoAttributes,
  NavigatorEnvVersion,
  NavigatorOpenType,
  NavigatorQuery,
  NavigatorTarget,
} from '@lingxia/elements';

export interface LxVideoProps {
  id?: string;
  src?: string;
  poster?: string;
  objectFit?: 'cover' | 'contain' | 'fill' | 'fit';
  contentRotate?: 0 | 90 | 180 | 270;
  autoplay?: boolean;
  loop?: boolean;
  muted?: boolean;
  controls?: boolean;
  progressBar?: boolean;
  live?: boolean;
  volume?: string | number;
  qualities?: Array<{ label: string; url?: string }>;
  playbackRates?: number[];
  class?: string;
  style?: CSSProperties;
  onPlayRequest?: (event: Event) => void;
  onPlay?: (event: Event) => void;
  onPlaying?: (event: Event) => void;
  onPause?: (event: Event) => void;
  onStop?: (event: Event) => void;
  onEnded?: (event: Event) => void;
  onTimeUpdate?: (event: Event) => void;
  onError?: (event: Event) => void;
  onLoadedMetadata?: (event: Event) => void;
  onFullscreenChange?: (event: Event) => void;
  onWaiting?: (event: Event) => void;
  onQualityChange?: (event: Event) => void;
  onRateChange?: (event: Event) => void;
  pageBindings?: Record<string, string>;
}

export interface LxNativeRootProps {
  id?: string;
  automationId?: string;
  class?: string;
  style?: CSSProperties;
  fullscreenScope?: 'root' | 'none';
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  hiddenTransition?: 'none' | 'fade';
}

export interface LxNativeViewProps {
  id?: string;
  class?: string;
  style?: CSSProperties;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  role?: 'group' | 'region' | 'status' | 'presentation' | 'none';
}

export interface LxNativeCoverProps extends LxNativeViewProps {
  scrim?: 'none' | 'top' | 'bottom' | 'full';
  scrimOpacity?: number;
}

export interface LxNativeTextProps {
  id?: string;
  class?: string;
  style?: CSSProperties;
  maxLines?: number;
  dir?: 'ltr' | 'rtl' | 'auto';
}

export interface LxNativeButtonProps {
  id?: string;
  class?: string;
  style?: CSSProperties;
  label?: string;
  icon?: string | { resource: unknown };
  intent?: 'neutral' | 'accent' | 'destructive';
  emphasis?: 'primary' | 'secondary' | 'quiet';
  size?: 'compact' | 'regular';
}

type LxMediaSwiperItem =
  | { id?: string; type: 'image'; src: string }
  | {
      id?: string;
      type: 'video';
      src: string;
      poster?: string;
      controls?: boolean;
      muted?: boolean;
    };

export interface LxMediaSwiperProps {
  id?: string;
  items?: LxMediaSwiperItem[];
  index?: number;
  initialIndex?: number;
  loop?: boolean;
  autoplay?: boolean;
  interval?: number;
  animation?: 'slide' | 'none';
  animationDuration?: number;
  direction?: 'horizontal' | 'vertical';
  contentRotate?: 0 | 90 | 180 | 270;
  objectFit?: 'cover' | 'contain' | 'fill' | 'fit';
  controls?: boolean;
  muted?: boolean;
  dots?: boolean | { color?: string; activeColor?: string };
  swipeEnabled?: boolean;
  peek?: number | { previous?: number; next?: number };
  class?: string;
  style?: CSSProperties;
  onChange?: (event: Event) => void;
  onTransitionEnd?: (event: Event) => void;
  onEndReached?: (event: Event) => void;
  onTap?: (event: Event) => void;
  onVideoEnded?: (event: Event) => void;
  onError?: (event: Event) => void;
  pageBindings?: Record<string, string>;
}

type IsExact<Left, Right> =
  (<Value>() => Value extends Left ? 1 : 2) extends
  (<Value>() => Value extends Right ? 1 : 2)
    ? (<Value>() => Value extends Right ? 1 : 2) extends
      (<Value>() => Value extends Left ? 1 : 2)
      ? true
      : false
    : false;
type AssertExact<Value extends true> = Value;
type _VideoPropsMatchElements = AssertExact<IsExact<
  Omit<LxVideoAttributes, 'ref' | 'className' | 'style'>,
  Omit<LxVideoProps, 'class' | 'style'>
>>;
type _MediaSwiperPropsMatchElements = AssertExact<IsExact<
  Omit<LxMediaSwiperAttributes, 'ref' | 'className' | 'style'>,
  Omit<LxMediaSwiperProps, 'class' | 'style'>
>>;

export interface LxPickerProps {
  columns?: string[][] | [string[], Record<string, string[]>];
  mode?: 'date' | 'time';
  start?: string;
  end?: string;
  fields?: 'year' | 'month' | 'day' | 'range';
  modelValue?: string | string[];
  placeholder?: string;
  class?: string;
  style?: CSSProperties;
  disabled?: boolean;
  cancelText?: string;
  cancelTextColor?: string;
  cancelButtonColor?: string;
  confirmText?: string;
  confirmTextColor?: string;
  confirmButtonColor?: string;
  pageBindings?: Record<string, string>;
}

export interface LxNavigatorProps {
  url?: string;
  page?: string;
  openType?: NavigatorOpenType;
  target?: NavigatorTarget;
  delta?: number;
  query?: NavigatorQuery;
  appId?: string;
  envVersion?: NavigatorEnvVersion;
  targetVersion?: string;
  phoneNumber?: string;
  hoverClass?: string;
  hoverStopPropagation?: boolean;
  hoverStartTime?: number;
  hoverStayTime?: number;
  class?: string;
  style?: CSSProperties;
}



export type { LxNavigatorEvent };
