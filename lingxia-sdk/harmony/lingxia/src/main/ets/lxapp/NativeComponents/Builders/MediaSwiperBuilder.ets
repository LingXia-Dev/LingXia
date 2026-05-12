/**
 * MediaSwiperBuilder - Builder for media-swiper.native component on Harmony.
 *
 * Uses ArkUI Swiper for paging. Each item is either an image (Image) or a
 * video (LxNativeVideoPlayer) rendered with a stable per-item componentId so
 * the underlying native player survives re-renders.
 */
import { NativeComponentParams, registerNativeComponentBuilder } from '../NativeComponentNodeController';
import { NativeComponentRecord, NativeComponentValue } from '../NativeComponentTypes';
import { onNativeComponentEventFromBuilder } from '../NativeBridgeInternal';
import {
  LxNativeVideoPlayer,
  LxNativeVideoConfig,
  VideoEventDetail,
  VideoErrorDetail,
} from '../../media/LxNativeVideoPlayer';
import { resolveLxUriForFileAccess } from '../../media/LxUri';
import { hilog } from '@kit.PerformanceAnalysisKit';

const SWIPER_DOMAIN: number = 0xA000;
// Tag includes the `lingxia` substring so it survives the standard
// `hdc hilog | grep lingxia` filter used during development debugging.
const SWIPER_TAG: string = 'LingXia.MediaSwiper';

const ALLOWED_ERROR_CODES: string[] = [
  'not_found',
  'network',
  'decode',
  'unsupported_format',
  'permission_denied',
  'unknown',
];

class SwiperItem {
  id: string = '';
  type: 'image' | 'video' = 'image';
  src: string = '';
  poster?: string;
  controls?: boolean;
  muted?: boolean;
}

class DotsConfig {
  enabled: boolean = false;
  color: string = '#66FFFFFF';
  activeColor: string = '#FFFFFFFF';
}

interface DotsRaw {
  color?: string;
  activeColor?: string;
}

interface RawItem {
  id?: string;
  type?: string;
  src?: string;
  poster?: string;
  controls?: boolean;
  muted?: boolean;
}

function parseItems(raw: NativeComponentValue): SwiperItem[] {
  if (raw === undefined || raw === null) {
    return [];
  }
  // Defensive: ESObject arrays from JSON.parse aren't always picked up by
  // `Array.isArray` under ArkTS strict mode, so fall back to a duck-typed
  // length check before bailing.
  const looksLikeArray =
    Array.isArray(raw) ||
    (typeof raw === 'object' &&
      typeof (raw as Record<string, NativeComponentValue>).length === 'number');
  if (!looksLikeArray) {
    hilog.warn(SWIPER_DOMAIN, SWIPER_TAG,
      `parseItems: raw is not array-like, type=${typeof raw}`);
    return [];
  }
  const list = raw as NativeComponentValue[];
  const out: SwiperItem[] = [];
  const len = list.length;
  for (let i = 0; i < len; i += 1) {
    const entry = list[i];
    const index = i;
    if (entry === undefined || entry === null || typeof entry !== 'object') {
      hilog.warn(SWIPER_DOMAIN, SWIPER_TAG,
        `parseItems: dropped index=${i} reason=not-object type=${typeof entry}`);
      continue;
    }
    const obj = entry as RawItem;
    // Use String(...) coercion rather than `typeof === 'string'` gates because
    // ArkTS strict mode property reads through an interface cast can return
    // a wrapped value whose `typeof` is `'object'` even when the underlying
    // value is logically a string — that's what was dropping every item before.
    const rawType: string | undefined = obj.type;
    const rawSrc: string | undefined = obj.src;
    const typeStr = (rawType !== undefined && rawType !== null) ? String(rawType) : '';
    const srcStr = (rawSrc !== undefined && rawSrc !== null) ? String(rawSrc) : '';
    if (typeStr !== 'image' && typeStr !== 'video') {
      hilog.warn(SWIPER_DOMAIN, SWIPER_TAG,
        `parseItems: dropped index=${i} reason=bad-type typeStr=${typeStr}`);
      continue;
    }
    const trimmed = srcStr.trim();
    if (trimmed.length === 0) {
      hilog.warn(SWIPER_DOMAIN, SWIPER_TAG,
        `parseItems: dropped index=${i} reason=empty-src`);
      continue;
    }
    const item = new SwiperItem();
    item.type = (typeStr === 'video') ? 'video' : 'image';
    item.src = trimmed;
    const rawId: string | undefined = obj.id;
    const idStr = (rawId !== undefined && rawId !== null) ? String(rawId) : '';
    item.id = (idStr.length > 0) ? idStr : `${item.type}:${trimmed}:${index}`;
    const rawPoster: string | undefined = obj.poster;
    if (rawPoster !== undefined && rawPoster !== null) {
      const p = String(rawPoster);
      if (p.length > 0) item.poster = p;
    }
    if (typeof obj.controls === 'boolean') item.controls = obj.controls;
    if (typeof obj.muted === 'boolean') item.muted = obj.muted;
    out.push(item);
  }
  hilog.info(SWIPER_DOMAIN, SWIPER_TAG,
    `parseItems: input length=${len} output length=${out.length}`);
  return out;
}

function parseDots(raw: NativeComponentValue, previous: DotsConfig): DotsConfig {
  if (raw === undefined || raw === null) {
    return previous;
  }
  if (typeof raw === 'boolean') {
    const next = new DotsConfig();
    next.enabled = raw;
    next.color = previous.color;
    next.activeColor = previous.activeColor;
    return next;
  }
  if (typeof raw === 'object') {
    const obj = raw as DotsRaw;
    const next = new DotsConfig();
    next.enabled = true;
    next.color = (typeof obj.color === 'string' && obj.color.length > 0) ? obj.color : previous.color;
    next.activeColor = (typeof obj.activeColor === 'string' && obj.activeColor.length > 0)
      ? obj.activeColor : previous.activeColor;
    return next;
  }
  return previous;
}

function normalizeRotate(value: NativeComponentValue): number {
  const n = (typeof value === 'number') ? value
    : (typeof value === 'string' ? Number.parseInt(value, 10) : NaN);
  if (n === 0 || n === 90 || n === 180 || n === 270) return n;
  return 0;
}

/**
 * Convert CSS color notation to an `rgba(r, g, b, a)` string ArkUI parses
 * unambiguously. We avoid passing 8-digit hex (`#RRGGBBAA` vs `#AARRGGBB`)
 * because different ArkUI primitives interpret the byte order differently —
 * `.backgroundColor('#CC000000')` reads it as #AARRGGBB while `Circle().fill()`
 * has been observed to misinterpret CSS-style #RRGGBBAA, rendering
 * `#ffffff66` as opaque yellow (R=ff G=ff B=66, ignoring the trailing alpha).
 * Going through rgba() matches the pattern used by NavigationBar/TabBar and
 * sidesteps the ambiguity entirely.
 */
function cssColorToRgba(input: string): string {
  if (!input) return input;
  const s = input.trim();
  if (s.length === 0 || s.charAt(0) !== '#') return s;
  const hex = s.substring(1);
  let r = 0;
  let g = 0;
  let b = 0;
  let a = 1.0;
  if (hex.length === 3) {
    r = parseInt(hex.charAt(0) + hex.charAt(0), 16);
    g = parseInt(hex.charAt(1) + hex.charAt(1), 16);
    b = parseInt(hex.charAt(2) + hex.charAt(2), 16);
  } else if (hex.length === 6) {
    r = parseInt(hex.substring(0, 2), 16);
    g = parseInt(hex.substring(2, 4), 16);
    b = parseInt(hex.substring(4, 6), 16);
  } else if (hex.length === 8) {
    r = parseInt(hex.substring(0, 2), 16);
    g = parseInt(hex.substring(2, 4), 16);
    b = parseInt(hex.substring(4, 6), 16);
    a = parseInt(hex.substring(6, 8), 16) / 255;
  } else {
    return s;
  }
  if (!Number.isFinite(r) || !Number.isFinite(g) || !Number.isFinite(b)) return s;
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

interface PeekConfig {
  previous: number;
  next: number;
}

interface PeekRaw {
  previous?: number;
  next?: number;
}

function parsePeek(raw: NativeComponentValue): PeekConfig {
  const out: PeekConfig = { previous: 0, next: 0 };
  if (raw === undefined || raw === null) return out;
  if (typeof raw === 'number' && Number.isFinite(raw)) {
    const v = Math.max(0, raw);
    out.previous = v;
    out.next = v;
    return out;
  }
  if (typeof raw === 'object') {
    const obj = raw as PeekRaw;
    if (typeof obj.previous === 'number' && Number.isFinite(obj.previous)) {
      out.previous = Math.max(0, obj.previous);
    }
    if (typeof obj.next === 'number' && Number.isFinite(obj.next)) {
      out.next = Math.max(0, obj.next);
    }
  }
  return out;
}

function parseObjectFit(value: NativeComponentValue): string {
  if (typeof value !== 'string') return 'cover';
  const lower = value.toLowerCase();
  if (lower === 'cover' || lower === 'contain' || lower === 'fill' || lower === 'fit') {
    return lower;
  }
  return 'cover';
}

function arkImageFit(fit: string): ImageFit {
  if (fit === 'contain' || fit === 'fit') return ImageFit.Contain;
  if (fit === 'fill') return ImageFit.Fill;
  return ImageFit.Cover;
}

function normalizeErrorCode(code: string): string {
  const lower = (code || '').toLowerCase();
  return ALLOWED_ERROR_CODES.indexOf(lower) >= 0 ? lower : 'unknown';
}

function itemsEqual(a: SwiperItem[], b: SwiperItem[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    const x = a[i];
    const y = b[i];
    if (x.id !== y.id) return false;
    if (x.type !== y.type) return false;
    if (x.src !== y.src) return false;
    if ((x.poster ?? '') !== (y.poster ?? '')) return false;
    if (x.controls !== y.controls) return false;
    if (x.muted !== y.muted) return false;
  }
  return true;
}

/**
 * Per-page view used inside the Swiper. A dedicated @Component (rather than an
 * inline Stack with if/else) gives ArkUI's Swiper a fixed-shape, stable child
 * per page. Without this, the Swiper would only lay out the first page and
 * subsequent items rendered at zero size — which is what was producing the
 * "only one image shows after picking multiple" bug.
 */
@Component
struct MediaSwiperPageView {
  @Prop item: SwiperItem;
  @Prop imageSrc: string;
  @Prop videoPlayerId: string;
  @Prop videoConfig: LxNativeVideoConfig;
  @Prop objectFit: string;
  @Prop rotateDegrees: number;
  @Prop parentControls: boolean;
  onImageError: (code: string, message: string) => void = () => {};
  onVideoEvent: (event: string, detail: VideoEventDetail) => void = () => {};
  onTap: () => void = () => {};

  private classifyImageError(msg: string): string {
    const lower = msg.toLowerCase();
    if (lower.indexOf('not found') >= 0 || lower.indexOf('no such') >= 0 || lower.indexOf('404') >= 0) {
      return 'not_found';
    }
    if (lower.indexOf('network') >= 0 || lower.indexOf('timeout') >= 0 || lower.indexOf('connect') >= 0) {
      return 'network';
    }
    if (lower.indexOf('decode') >= 0 || lower.indexOf('format') >= 0) {
      return 'decode';
    }
    if (lower.indexOf('permission') >= 0 || lower.indexOf('denied') >= 0) {
      return 'permission_denied';
    }
    return 'unknown';
  }

  build() {
    Stack() {
      if (this.item.type === 'image') {
        // ArkUI's Image only understands file://, http(s)://, $r() resources.
        // lx:// URIs from chooseMedia must be resolved through the Rust bridge
        // first, otherwise the loader silently fails and the page renders blank.
        Image(this.imageSrc)
          .width('100%')
          .height('100%')
          .objectFit(arkImageFit(this.objectFit))
          .rotate({ angle: this.rotateDegrees })
          .onError((event: ESObject) => {
            const raw: ESObject = event?.message;
            const msg: string = (typeof raw === 'string') ? raw as string : '';
            const code = this.classifyImageError(msg);
            this.onImageError(code, msg.length > 0 ? msg : 'image load failed');
          })
          .onClick(() => {
            this.onTap();
          })
      } else {
        LxNativeVideoPlayer({
          playerId: this.videoPlayerId,
          config: this.videoConfig,
          onEvent: (event: string, detail: VideoEventDetail) => {
            this.onVideoEvent(event, detail);
          },
        })
          .width('100%')
          .height('100%')
          .onClick(() => {
            // Only forward as swiper tap when controls are off so native
            // controls keep their own play/pause/seek interactions.
            const effectiveControls = (this.item.controls !== undefined) ? this.item.controls : this.parentControls;
            if (!effectiveControls) {
              this.onTap();
            }
          })
      }
    }
    .width('100%')
    .height('100%')
  }
}

@Component
struct NativeMediaSwiperComponent {
  @Prop @Watch('onParamsChange') params: NativeComponentParams;
  private componentId: string = '';
  private swiperController: SwiperController = new SwiperController();
  @State items: SwiperItem[] = [];
  @State currentIndex: number = 0;
  @State loop: boolean = false;
  @State autoplay: boolean = false;
  @State interval: number = 5000;
  // Renamed away from ArkUI's built-in component method names (animation/direction/rotate),
  // which ArkTS strict mode treats as illegal property shadowing.
  @State animationMode: string = 'slide';
  @State animationDurationMs: number = 300;
  @State swipeDirection: string = 'horizontal';
  @State rotateDegrees: number = 0;
  @State objectFit: string = 'cover';
  @State controls: boolean = false;
  @State muted: boolean = true;
  @State dots: DotsConfig = new DotsConfig();
  @State swipeEnabled: boolean = true;
  @State peekPrevious: number = 0;
  @State peekNext: number = 0;
  // ArkUI's Swiper doesn't reliably reconcile modifier updates (loop, vertical,
  // prevMargin/nextMargin, etc.) through BuilderNode.update — even when the
  // underlying @State changes, the Swiper UI keeps rendering with the old
  // values until *something else* forces a redraw. swiperRevision bumps on any
  // such config change and is used as a ForEach key over a single-element
  // array wrapping the Swiper, forcing the whole subtree to recreate so the
  // new modifier values take effect.
  @State swiperRevision: number = 0;
  private initialized: boolean = false;
  private lastSettledIndex: number = 0;
  private suppressNextChange: boolean = false;
  private pendingTransitionPrevious: number | null = null;
  private pendingTransitionSource: string = 'touch';
  private nextChangeSource: string = 'touch';
  private gestureStarted: boolean = false;
  // Last imperative-API command sequence we've already dispatched. The JS
  // element side bumps __cmdSeq each time next()/previous()/goToIndex() is
  // called and re-pushes props; we de-dup by sequence number so a stale
  // params snapshot never replays an old command.
  private lastDispatchedCmdSeq: number = 0;
  // Snapshot of the last props payload we ran applyParams against. ArkWeb fires
  // onParamsChange in cascades (layout settles, scroll, etc.) and parseDots /
  // parsePeek create fresh objects each call — without this short-circuit, every
  // call reassigns @State and triggers another render, which feeds back into more
  // applyParams calls (we observed ~15 calls in 200ms with identical props).
  private lastAppliedPropsHash: string = '';

  aboutToAppear(): void {
    if (this.initialized) return;
    this.initialized = true;
    if (this.params) {
      this.componentId = this.params.componentId || '';
    }
    this.applyParams(true);
  }

  onParamsChange(): void {
    // Internal SDK lifecycle commands (pause/resume/play/stop) come via the
    // updateNode(__command) mechanism in NativeComponentEmbed. Swiper doesn't
    // need them; absorb so they don't reach applyParams.
    const internalCommand = (this.params as ESObject)?.__command as string;
    if (internalCommand === 'pause' || internalCommand === 'resume' ||
        internalCommand === 'play' || internalCommand === 'stop') {
      return;
    }
    this.applyParams(false);
    // After regular props are applied, dispatch any imperative API command
    // delivered via the JS-side bridge fallback.
    this.dispatchImperativeCommandIfAny();
  }

  private dispatchImperativeCommandIfAny(): void {
    const props = this.params?.props;
    if (!props) return;
    const seqRaw = props.get('__cmdSeq');
    if (typeof seqRaw !== 'number' || seqRaw <= this.lastDispatchedCmdSeq) return;
    this.lastDispatchedCmdSeq = seqRaw;
    const name = props.get('__cmdName');
    if (typeof name !== 'string') return;
    if (name === 'next') {
      this.handleApiNext();
      return;
    }
    if (name === 'previous') {
      this.handleApiPrevious();
      return;
    }
    if (name === 'goToIndex') {
      const cmdParams = props.get('__cmdParams');
      const index = (cmdParams && typeof cmdParams === 'object')
        ? ((cmdParams as ESObject).index as number)
        : NaN;
      if (typeof index === 'number') {
        this.handleApiGoToIndex(index);
      }
    }
  }

  private applyParams(isInitial: boolean): void {
    const props = this.params?.props;
    if (!props) return;

    // Short-circuit when props haven't changed. ArkWeb fires onParamsChange in
    // cascades during layout settle; without this guard each repeat call
    // reassigns fresh DotsConfig/peek/items objects and triggers another render.
    const hash = props.toJSONString();
    if (!isInitial && hash === this.lastAppliedPropsHash) {
      return;
    }
    this.lastAppliedPropsHash = hash;
    hilog.info(SWIPER_DOMAIN, SWIPER_TAG,
      `[${this.componentId}] applyParams isInitial=${isInitial} propsHashLen=${hash.length}`);

    const previousItems = this.items;
    const previousIndex = this.currentIndex;
    const priorItem = previousItems[previousIndex];
    const previousDirection = this.swipeDirection;
    const previousPeekPrevious = this.peekPrevious;
    const previousPeekNext = this.peekNext;
    const previousLoop = this.loop;
    const previousAutoplay = this.autoplay;
    const previousInterval = this.interval;
    const previousAnimationMode = this.animationMode;
    const previousAnimationDurationMs = this.animationDurationMs;
    const previousSwipeEnabled = this.swipeEnabled;

    const nextItems = parseItems(props.get('items'));
    const itemsChanged = !itemsEqual(previousItems, nextItems);

    const indexProp = props.get('index');
    const initialIndexProp = props.get('initialIndex');
    const indexNumber = (typeof indexProp === 'number') ? indexProp : undefined;
    const initialIndexNumber = (typeof initialIndexProp === 'number') ? initialIndexProp : 0;

    this.loop = (typeof props.get('loop') === 'boolean') ? props.get('loop') as boolean : this.loop;
    this.autoplay = (typeof props.get('autoplay') === 'boolean') ? props.get('autoplay') as boolean : this.autoplay;
    const intervalRaw = props.get('interval');
    if (typeof intervalRaw === 'number') {
      this.interval = Math.max(500, intervalRaw);
    }
    const animationRaw = props.get('animation');
    if (animationRaw === 'none' || animationRaw === 'slide') {
      this.animationMode = animationRaw;
    }
    const durationRaw = props.get('animationDuration');
    if (typeof durationRaw === 'number') {
      this.animationDurationMs = Math.max(0, durationRaw);
    }
    const directionRaw = props.get('direction');
    if (directionRaw === 'horizontal' || directionRaw === 'vertical') {
      this.swipeDirection = directionRaw;
    }
    this.rotateDegrees = normalizeRotate(props.get('contentRotate'));
    this.objectFit = parseObjectFit(props.get('objectFit'));
    this.controls = (typeof props.get('controls') === 'boolean') ? props.get('controls') as boolean : this.controls;
    this.muted = (typeof props.get('muted') === 'boolean') ? props.get('muted') as boolean : this.muted;
    this.dots = parseDots(props.get('dots'), this.dots);
    const swipeEnabledRaw = props.get('swipeEnabled');
    this.swipeEnabled = (typeof swipeEnabledRaw === 'boolean') ? swipeEnabledRaw : this.swipeEnabled;

    const peek = parsePeek(props.get('peek'));
    this.peekPrevious = peek.previous;
    this.peekNext = peek.next;

    // Bump swiperRevision whenever any prop the Swiper applies as a *modifier*
    // changes. ArkUI Swiper's modifier reconciliation through BuilderNode.update
    // is unreliable — toggling loop, switching axis, or adjusting peek won't
    // visibly take effect unless we force the Swiper subtree to recreate.
    // Items/index/objectFit/controls/muted/dots are intentionally excluded:
    // those reconcile correctly through ForEach reactivity or the per-item
    // MediaSwiperPageView @Prop pathway, and rebuilding on every item change
    // would needlessly tear down active video players.
    const swiperConfigChanged =
      previousDirection !== this.swipeDirection
      || previousPeekPrevious !== this.peekPrevious
      || previousPeekNext !== this.peekNext
      || previousLoop !== this.loop
      || previousAutoplay !== this.autoplay
      || previousInterval !== this.interval
      || previousAnimationMode !== this.animationMode
      || previousAnimationDurationMs !== this.animationDurationMs
      || previousSwipeEnabled !== this.swipeEnabled;
    if (swiperConfigChanged) {
      this.swiperRevision += 1;
      this.suppressNextChange = true;
      this.lastSettledIndex = this.currentIndex;
      hilog.info(SWIPER_DOMAIN, SWIPER_TAG,
        `[${this.componentId}] swiper config changed; revision=${this.swiperRevision}`);
    }

    if (itemsChanged) {
      this.items = nextItems;
      hilog.info(SWIPER_DOMAIN, SWIPER_TAG,
        `[${this.componentId}] items changed previousLen=${previousItems.length} nextLen=${nextItems.length}`);
      let targetIndex: number;
      if (typeof indexNumber === 'number') {
        targetIndex = this.clampIndex(indexNumber);
      } else if (priorItem && previousItems.length > 0) {
        const matched = nextItems.findIndex((x: SwiperItem): boolean => x.id === priorItem.id);
        targetIndex = matched >= 0 ? matched : this.clampIndex(initialIndexNumber);
      } else {
        targetIndex = this.clampIndex(initialIndexNumber);
      }
      this.suppressNextChange = true;
      this.currentIndex = targetIndex;
      this.lastSettledIndex = targetIndex;
    } else if (typeof indexNumber === 'number') {
      const resolved = this.clampIndex(indexNumber);
      if (resolved !== this.currentIndex) {
        this.suppressNextChange = true;
        this.currentIndex = resolved;
        this.lastSettledIndex = resolved;
      }
    }

    if (isInitial && typeof indexNumber !== 'number') {
      this.currentIndex = this.clampIndex(initialIndexNumber);
      this.lastSettledIndex = this.currentIndex;
    }

  }

  private clampIndex(value: number): number {
    if (this.items.length <= 0) return 0;
    if (!Number.isFinite(value)) return 0;
    const last = this.items.length - 1;
    return Math.max(0, Math.min(last, Math.trunc(value)));
  }

  // MARK: - Imperative API

  private handleApiNext(): void {
    const count = this.items.length;
    if (count === 0) return;
    const target = this.currentIndex + 1;
    if (target >= count) {
      if (this.loop && count > 1) {
        this.transitionTo(0, 'api');
      } else {
        this.emitEndReached(this.currentIndex, 'api');
      }
      return;
    }
    this.transitionTo(target, 'api');
  }

  private handleApiPrevious(): void {
    const count = this.items.length;
    if (count === 0) return;
    const target = this.currentIndex - 1;
    if (target < 0) {
      if (this.loop && count > 1) {
        this.transitionTo(count - 1, 'api');
      } else {
        this.emitEndReached(this.currentIndex, 'api');
      }
      return;
    }
    this.transitionTo(target, 'api');
  }

  private handleApiGoToIndex(index: number): void {
    if (!Number.isFinite(index)) return;
    const target = Math.trunc(index);
    if (target < 0 || target >= this.items.length) return;
    if (target === this.currentIndex) return;
    this.transitionTo(target, 'api');
  }

  private transitionTo(target: number, source: string): void {
    if (target === this.currentIndex) return;
    const previous = this.currentIndex;
    this.nextChangeSource = source;
    this.pendingTransitionPrevious = previous;
    this.pendingTransitionSource = source;
    this.currentIndex = target;
    // ArkUI Swiper observes @State currentIndex via .index() binding and animates.
    // It will fire onChange (we tag via nextChangeSource) and onAnimationEnd.
  }

  // MARK: - Events

  private handleSwiperChange(index: number): void {
    if (this.suppressNextChange) {
      this.suppressNextChange = false;
      this.gestureStarted = false;
      this.lastSettledIndex = index;
      this.currentIndex = index;
      return;
    }
    const previous = this.currentIndex;
    // Source priority: a touch in flight always wins (even if autoplay is also
    // enabled) so that user gestures never get mislabeled as 'autoplay'. API
    // commands route through transitionTo which pre-sets pendingTransitionSource.
    let source: string;
    if (this.gestureStarted) {
      source = 'touch';
    } else if (this.pendingTransitionPrevious !== null) {
      source = this.pendingTransitionSource;
    } else if (this.autoplay) {
      source = 'autoplay';
    } else {
      source = 'touch';
    }
    this.gestureStarted = false;
    if (this.pendingTransitionPrevious === null) {
      this.pendingTransitionPrevious = previous;
      this.pendingTransitionSource = source;
    }
    this.currentIndex = index;
    this.emitChange(index, this.pendingTransitionPrevious as number, source);
    // Pause the prior video and resume the new one — without this, the cached
    // adjacent video keeps playing in the background and the newly-active
    // video stays paused (its cfg.autoplay was false at mount).
    this.syncActiveVideoPlayback();
  }

  private handleAnimationEnd(): void {
    const settled = this.currentIndex;
    let settledSource: string | null = null;
    if (this.pendingTransitionPrevious !== null && this.pendingTransitionPrevious !== settled) {
      const previous = this.pendingTransitionPrevious as number;
      settledSource = this.pendingTransitionSource;
      this.lastSettledIndex = settled;
      this.emitTransitionEnd(settled, previous, settledSource);
      this.pendingTransitionPrevious = null;
      this.pendingTransitionSource = 'touch';
    } else {
      this.lastSettledIndex = settled;
    }
    // Re-assert playback ownership at settle time — covers cases where the
    // newly active video's player wasn't yet created at handleSwiperChange.
    this.syncActiveVideoPlayback();
    // Spec: endreached fires only when autoplay/api attempt to move past an edge,
    // never for manual touch settling on the edge. Re-emit only when this animated
    // transition was driven by autoplay AND landed on the last index with loop=false.
    if (
      settledSource === 'autoplay' &&
      !this.loop &&
      settled === this.items.length - 1
    ) {
      this.emitEndReached(settled, 'autoplay');
    }
  }

  /**
   * Build the detail payload as a plain ESObject literal. We used to allocate
   * `SwiperEventDetail` class instances and assign fields imperatively, but
   * ArkTS strict mode class instances don't expose their declared fields via
   * `Object.keys()` the way plain objects do — so the dispatcher's
   * `normalizeValue` walked an empty key list and produced `{}`. That landed
   * on the JS side as `event.detail = {}` (so `detail.index` was `undefined`),
   * which is the `video ended undefined` symptom seen in the showcase.
   */
  private buildEventDetail(
    index: number,
    previousIndex?: number,
    source?: string,
    code?: string,
    message?: string
  ): ESObject {
    const out: ESObject = {} as ESObject;
    out.index = index;
    out.item = this.itemPayload(index);
    if (previousIndex !== undefined) out.previousIndex = previousIndex;
    if (source !== undefined) out.source = source;
    if (code !== undefined) out.code = code;
    if (message !== undefined) out.message = message;
    return out;
  }

  private emitChange(index: number, previousIndex: number, source: string): void {
    // Defensive: if pendingTransitionPrevious / currentIndex got out of sync
    // (e.g. transitionTo + JS-driven prop change racing), the previousIndex
    // can collapse to the same value as the new index. That isn't a real
    // change — silently drop so the showcase never logs `change N → N`.
    if (previousIndex === index) {
      return;
    }
    onNativeComponentEventFromBuilder(this.componentId, 'change',
      this.buildEventDetail(index, previousIndex, source));
  }

  private emitTransitionEnd(index: number, previousIndex: number, source: string): void {
    onNativeComponentEventFromBuilder(this.componentId, 'transitionend',
      this.buildEventDetail(index, previousIndex, source));
  }

  private emitEndReached(index: number, source: string): void {
    onNativeComponentEventFromBuilder(this.componentId, 'endreached',
      this.buildEventDetail(index, undefined, source));
  }

  private emitTap(index: number): void {
    onNativeComponentEventFromBuilder(this.componentId, 'tap',
      this.buildEventDetail(index));
  }

  private emitVideoEnded(index: number): void {
    onNativeComponentEventFromBuilder(this.componentId, 'videoended',
      this.buildEventDetail(index));
  }

  private emitError(index: number, code: string, message: string): void {
    onNativeComponentEventFromBuilder(this.componentId, 'error',
      this.buildEventDetail(index, undefined, undefined, normalizeErrorCode(code), message));
  }

  private itemPayload(index: number): ESObject {
    if (index < 0 || index >= this.items.length) return {} as ESObject;
    const item = this.items[index];
    // ArkTS strict mode forbids untyped object literals against Record<>.
    // Build the payload as a plain ESObject and assign fields imperatively.
    const out: ESObject = {} as ESObject;
    out.id = item.id;
    out.type = item.type;
    out.src = item.src;
    if (item.poster !== undefined) out.poster = item.poster;
    if (item.controls !== undefined) out.controls = item.controls;
    if (item.muted !== undefined) out.muted = item.muted;
    return out;
  }

  // MARK: - Per-item handlers

  private handlePageTap(index: number): void {
    this.emitTap(index);
  }

  private handleVideoEvent(index: number, event: string, detail: VideoEventDetail): void {
    if (event === 'ended') {
      // Spec: native does NOT auto-advance on video end — business JS decides
      // whether to step, loop, or stop. Emitting + advancing here was producing
      // a dual-driver race with showcase logic (native transitionTo + JS
      // setData both targeting the same next-index) which surfaced as bogus
      // `change X → X` logs and double playback transitions.
      this.emitVideoEnded(index);
      return;
    }
    if (event === 'error') {
      const err = detail as VideoErrorDetail;
      this.emitError(index, err?.code ?? 'unknown', err?.message ?? 'video error');
      return;
    }
  }

  // MARK: - Video config

  private buildVideoConfig(item: SwiperItem, index: number): LxNativeVideoConfig {
    const cfg = new LxNativeVideoConfig();
    cfg.src = item.src;
    if (item.poster !== undefined) cfg.poster = item.poster;
    // Only the active page autoplays at mount time — adjacent cached items
    // share the swiper but must not play in the background. Subsequent
    // transitions are handled imperatively in syncActiveVideoPlayback().
    cfg.autoplay = (index === this.currentIndex);
    cfg.loop = false;
    cfg.muted = (item.muted !== undefined) ? item.muted : this.muted;
    cfg.volume = 1.0;
    const effectiveControls = (item.controls !== undefined) ? item.controls : this.controls;
    cfg.controls = effectiveControls;
    cfg.progressBar = effectiveControls;
    cfg.live = false;
    cfg.cornerRadius = 0;
    cfg.objectFit = this.objectFit;
    cfg.rotate = this.rotateDegrees;
    return cfg;
  }

  private videoComponentId(item: SwiperItem): string {
    return `${this.componentId}-video-${item.id}`;
  }

  /**
   * Resume the active video and pause every other mounted video. Called after
   * every onChange / onAnimationEnd / applyParams index change. `playStatic` /
   * `pauseStatic` are no-ops on items whose native player isn't created yet —
   * those items rely on their `cfg.autoplay` flag at mount time.
   */
  private syncActiveVideoPlayback(): void {
    if (this.items.length === 0) return;
    this.items.forEach((it: SwiperItem, idx: number) => {
      if (it.type !== 'video') return;
      const pid = this.videoComponentId(it);
      if (idx === this.currentIndex) {
        LxNativeVideoPlayer.playStatic(pid);
      } else {
        LxNativeVideoPlayer.pauseStatic(pid);
      }
    });
  }

  /**
   * Resolve an image item's src for ArkUI Image. Logs every resolution so the
   * "blank image" failure mode is observable in hilog without device-side
   * debugging — common cause is `getCurrentLxapp().appid` being empty, in which
   * case `resolveLxUriForFileAccess` returns the lx:// URI unchanged and Image
   * silently fails to load.
   */
  private resolveImageSrc(src: string, index: number): string {
    const resolved = resolveLxUriForFileAccess(src);
    return resolved;
  }

  @Builder
  private swiperBlock(verticalAxis: boolean) {
    Swiper(this.swiperController) {
      ForEach(this.items, (item: SwiperItem, index: number) => {
        // Each page is its own @Component (matches the working pattern in
        // MediaPreviewOverlay's MediaViewer). Previously we inlined a Stack
        // with `if (image) Image(...) else Stack { Video }` directly inside
        // ForEach, and ArkUI's Swiper only rendered the first page — every
        // subsequent page produced a zero-sized child. Wrapping each page in
        // a dedicated component gives the Swiper a stable, fixed shape per
        // page so all N items are laid out correctly.
        MediaSwiperPageView({
          item: item,
          imageSrc: item.type === 'image' ? this.resolveImageSrc(item.src, index) : '',
          videoPlayerId: item.type === 'video' ? this.videoComponentId(item) : '',
          videoConfig: item.type === 'video' ? this.buildVideoConfig(item, index) : new LxNativeVideoConfig(),
          objectFit: this.objectFit,
          rotateDegrees: this.rotateDegrees,
          parentControls: this.controls,
          onImageError: (code: string, msg: string) => {
            hilog.warn(SWIPER_DOMAIN, SWIPER_TAG,
              `[${this.componentId}] image[${index}] error code=${code} msg=${msg}`);
            this.emitError(index, code, msg);
          },
          onVideoEvent: (event: string, detail: VideoEventDetail) => {
            this.handleVideoEvent(index, event, detail);
          },
          onTap: () => {
            this.handlePageTap(index);
          },
        })
          .width('100%')
          .height('100%')
      }, (item: SwiperItem): string => item.id)
    }
    .width('100%')
    .height('100%')
    .index(this.currentIndex)
    .indicator(false)
    .loop(this.loop)
    .autoPlay(this.autoplay)
    .interval(this.interval)
    .duration(this.animationMode === 'none' ? 0 : this.animationDurationMs)
    .vertical(verticalAxis)
    .disableSwipe(!this.swipeEnabled)
    .cachedCount(this.items.length > 1 ? 1 : 0)
    .displayCount(1)
    .prevMargin(`${this.peekPrevious}vp`)
    .nextMargin(`${this.peekNext}vp`)
    .onTouch((event: TouchEvent) => {
      if (event.type === TouchType.Down) {
        this.gestureStarted = true;
      }
    })
    .onChange((index: number) => {
      this.handleSwiperChange(index);
    })
    .onAnimationEnd((_index: number, _extraInfo: SwiperAnimationEvent) => {
      this.handleAnimationEnd();
    })
  }

  build() {
    Stack({ alignContent: this.swipeDirection === 'vertical' ? Alignment.End : Alignment.Bottom }) {
      if (this.items.length > 0) {
        // Wrap the Swiper in a single-item ForEach keyed by swiperRevision so
        // that any Swiper-config change (loop/direction/peek/...) destroys the
        // old Swiper subtree and rebuilds it with the new modifiers applied.
        // See the comment on swiperRevision for why this dance is necessary.
        ForEach([0] as number[], (_: number) => {
          if (this.swipeDirection === 'horizontal') {
            this.swiperBlock(false)
          } else {
            this.swiperBlock(true)
          }
        }, (_: number): string => `swiper-rev-${this.swiperRevision}`)
      }

      if (this.dots.enabled && this.items.length > 1) {
        if (this.swipeDirection === 'vertical') {
          Column() {
            ForEach(this.items, (dotItem: SwiperItem, index: number) => {
              Circle()
                .width(6)
                .height(6)
                .fill(cssColorToRgba(index === this.currentIndex ? this.dots.activeColor : this.dots.color))
                .margin({ top: 4, bottom: 4 })
            })
          }
          .margin({ right: 12 })
        } else {
          Row() {
            ForEach(this.items, (dotItem: SwiperItem, index: number) => {
              Circle()
                .width(6)
                .height(6)
                .fill(cssColorToRgba(index === this.currentIndex ? this.dots.activeColor : this.dots.color))
                .margin({ left: 4, right: 4 })
            })
          }
          .margin({ bottom: 12 })
        }
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }
}

@Builder
function mediaSwiperBuilder(params: NativeComponentParams) {
  NativeMediaSwiperComponent({ params: params })
}

/**
 * Register media-swiper builder for same-level rendering.
 */
export function registerMediaSwiperBuilder(): void {
  registerNativeComponentBuilder('media-swiper.native', wrapBuilder(mediaSwiperBuilder));
}
