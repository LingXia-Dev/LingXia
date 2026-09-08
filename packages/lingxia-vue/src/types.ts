import type { CSSProperties } from 'vue';
import type {
  LxMediaSwiperAttributes,
  LxNavigatorEvent,
  LxVideoAttributes,
  LxVideoEventHandlers,
  LxVideoEventPayloads,
  NativeActionIcon,
  NativeStyleProperty,
  NavigatorEnvVersion,
  NavigatorOpenType,
  NavigatorQuery,
  NavigatorTarget,
} from '@lingxia/elements';

export type NativeStyle = Omit<Pick<CSSProperties, NativeStyleProperty>, "borderStyle"> & {
  borderStyle?: "solid" | "none";
};

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
  style?: NativeStyle;
  onPlayRequest?: (payload: LxVideoEventPayloads["onPlayRequest"]) => void;
  onPlay?: (payload: LxVideoEventPayloads["onPlay"]) => void;
  onPlaying?: (payload: LxVideoEventPayloads["onPlaying"]) => void;
  onPause?: (payload: LxVideoEventPayloads["onPause"]) => void;
  onStop?: (payload: LxVideoEventPayloads["onStop"]) => void;
  onEnded?: (payload: LxVideoEventPayloads["onEnded"]) => void;
  onWaiting?: (payload: LxVideoEventPayloads["onWaiting"]) => void;
  onTimeUpdate?: (payload: LxVideoEventPayloads["onTimeUpdate"]) => void;
  onError?: (payload: LxVideoEventPayloads["onError"]) => void;
  onLoadedMetadata?: (payload: LxVideoEventPayloads["onLoadedMetadata"]) => void;
  onFullscreenChange?: (payload: LxVideoEventPayloads["onFullscreenChange"]) => void;
  onQualityChange?: (payload: LxVideoEventPayloads["onQualityChange"]) => void;
  onRateChange?: (payload: LxVideoEventPayloads["onRateChange"]) => void;
  onVolumeChange?: (payload: LxVideoEventPayloads["onVolumeChange"]) => void;
  pageBindings?: Record<string, string>;
}

export interface LxNativeNodeProps {
  id?: string;
  automationId?: string;
  class?: string;
  style?: NativeStyle;
  pointerEvents?: 'auto' | 'none' | 'box-only' | 'box-none';
  hidden?: boolean;
  'aria-label'?: string;
  'aria-description'?: string;
  'aria-hidden'?: boolean;
}

export interface LxNativeRootProps extends LxNativeNodeProps {
}

export interface LxNativeViewProps extends LxNativeNodeProps {
  role?: 'group' | 'region' | 'status' | 'presentation' | 'none';
}

export interface LxNativeCoverProps extends LxNativeViewProps {
  scrim?: 'none' | 'top' | 'bottom' | 'full';
  scrimOpacity?: number;
}

export interface LxNativeTextProps extends LxNativeNodeProps {
  maxLines?: number;
  dir?: 'ltr' | 'rtl' | 'auto';
  fontSize?: number | string;
  fontWeight?: number | string;
  lineHeight?: number | string;
  textAlign?: 'start' | 'center' | 'end';
  color?: string;
}

export interface LxNativeButtonProps extends LxNativeNodeProps {
  label?: string;
  icon?: NativeActionIcon;
  iconPosition?: 'start' | 'end';
  intent?: 'neutral' | 'accent' | 'destructive';
  emphasis?: 'primary' | 'secondary' | 'quiet';
  size?: 'compact' | 'regular';
  hitSlop?: number;
  disabled?: boolean;
  pressed?: boolean;
  expanded?: boolean;
  loading?: boolean;
  tabIndex?: 0 | -1;
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
  Omit<LxVideoAttributes, 'ref' | 'className' | 'style' | `on${string}`>,
  Omit<LxVideoProps, 'class' | 'style' | keyof LxVideoEventHandlers>
>>;
type _VideoHandlersMatchElements = AssertExact<IsExact<
  Pick<LxVideoProps, keyof LxVideoEventHandlers>,
  LxVideoEventHandlers
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
