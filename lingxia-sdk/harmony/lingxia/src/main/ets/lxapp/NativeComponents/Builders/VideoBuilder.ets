/**
 * VideoBuilder - Builder for video component in same-level rendering
 * Uses native XComponent + OH_AVPlayer for direct control
 */
import { display, window } from '@kit.ArkUI';
import { NativeComponentParams, registerNativeComponentBuilder } from '../NativeComponentNodeController';
import { NativeComponentRecord, NativeComponentValue, nativeComponentError } from '../NativeComponentTypes';
import { dispatchNativeComponentEventFromBuilder, getNodeControllerByComponentId } from '../NativeBridgeInternal';
import {
  LxNativeVideoPlayer,
  LxNativeVideoConfig,
  LxMediaQuality,
  VideoEventDetail
} from '../../media/LxNativeVideoPlayer';
import { onCallback } from 'liblingxia.so';
import { getComponentCallback } from '../NativeCallbackRegistry';

interface QualityProp {
  label?: string;
  url?: string;
}

function clearPropsContains(props: NativeComponentRecord, key: string): boolean {
  const raw = props.get('__clearProps');
  if (!Array.isArray(raw)) {
    return false;
  }
  return raw.some((item: NativeComponentValue): boolean => typeof item === 'string' && item === key);
}

/**
 * Video component wrapper for same-level rendering
 * Uses native XComponent + OH_AVPlayer for direct control
 */
@Component
struct NativeVideoComponent {
  @Prop @Watch('onParamsChange') params: NativeComponentParams;
  private componentId: string = '';
  private initialized: boolean = false;
  private cachedConfig: LxNativeVideoConfig | null = null;

  aboutToAppear(): void {
    // Only initialize once to avoid duplicate events
    if (this.initialized) {
      return;
    }
    this.initialized = true;

    // Cache componentId and eventSink from params
    if (this.params) {
      this.componentId = this.params.componentId || '';
    }
  }

  onParamsChange(): void {
    // Handle commands from updateNode
    const command = (this.params as ESObject)?.__command as string;
    if (command === 'pause') {
      LxNativeVideoPlayer.pauseStatic(this.componentId);
    } else if (command === 'stop') {
      LxNativeVideoPlayer.stopStatic(this.componentId);
    } else if (command === 'play') {
      LxNativeVideoPlayer.playStatic(this.componentId);
    } else if (command === 'resume') {
      LxNativeVideoPlayer.resumeStatic(this.componentId);
    }

  }

  private buildConfig(): LxNativeVideoConfig {
    const props = this.params?.props;

    // If props are missing but we have a cache, use it to prevent state loss
    if (!props) {
      return this.cachedConfig ?? new LxNativeVideoConfig();
    }

    const config: LxNativeVideoConfig = new LxNativeVideoConfig();

    // Parse source
    const src = props.get('src');
    if (typeof src === 'string') config.src = src;

    const poster = props.get('poster');
    if (typeof poster === 'string') config.poster = poster;

    const autoplay = props.get('autoplay');
    if (typeof autoplay === 'boolean') config.autoplay = autoplay;

    const loop = props.get('loop');
    if (typeof loop === 'boolean') config.loop = loop;

    const muted = props.get('muted');
    if (typeof muted === 'boolean') config.muted = muted;

    const volume = props.get('volume');
    if (typeof volume === 'number') config.volume = volume;

    const controls = props.get('controls');
    if (typeof controls === 'boolean') config.controls = controls;

    const progressBar = props.get('progressBar');
    if (typeof progressBar === 'boolean') config.progressBar = progressBar;

    const live = props.get('live');
    if (typeof live === 'boolean') config.live = live;

    const cornerRadius = props.get('cornerRadius');
    if (typeof cornerRadius === 'number') config.cornerRadius = cornerRadius;

    const objectFit = props.get('objectFit');
    if (!clearPropsContains(props, 'objectFit') && typeof objectFit === 'string') {
      config.objectFit = objectFit;
    }
    if (clearPropsContains(props, 'rotate')) {
      config.rotate = undefined;
    } else {
      const rotate = props.get('rotate');
      if (typeof rotate === 'number') {
        config.rotate = rotate;
      } else if (typeof rotate === 'string') {
        const parsed = Number.parseInt(rotate, 10);
        if (!Number.isNaN(parsed)) config.rotate = parsed;
      }
    }

    // Parse qualities
    const qualities = props.get('qualities');
    if (Array.isArray(qualities)) {
      config.qualities = qualities.map((entry: QualityProp): LxMediaQuality => {
        const quality = new LxMediaQuality();
        if (typeof entry.label === 'string') quality.label = entry.label;
        if (typeof entry.url === 'string') quality.url = entry.url;
        return quality;
      }).filter((q: LxMediaQuality): boolean => q.label.length > 0);
    }

    // Parse playbackRates (first is default)
    const playbackRates = props.get('playbackRates');
    if (Array.isArray(playbackRates)) {
      config.playbackRates = playbackRates.filter((r: number): boolean => typeof r === 'number' && !isNaN(r));
    }

    // Cache the valid config
    this.cachedConfig = config;
    return config;
  }

  private handleEvent(event: string, detail: VideoEventDetail): void {
    // Handle fullscreen by toggling NodeController state
    if (event === 'fullscreenchange') {
      // Keep native player alive while we rebuild into fullscreen/display mode
      LxNativeVideoPlayer.setKeepAlive(this.componentId, true);
      this.toggleFullscreen();
    }

    dispatchNativeComponentEventFromBuilder(this.componentId, event, detail);

    const callbackId = getComponentCallback(this.componentId);
    const shouldForwardToCallback =
      event === 'waiting' ||
      event === 'playrequest' ||
      event === 'play' ||
      event === 'playing' ||
      event === 'pause' ||
      event === 'stop' ||
      event === 'seeking' ||
      event === 'seeked' ||
      event === 'ended' ||
      event === 'error';
    if (callbackId && shouldForwardToCallback) {
      const payload = new NativeComponentRecord();
      payload.set('action', 'component.event');
      payload.set('id', this.componentId);
      payload.set('event', event);
      payload.set('detail', detail);
      payload.set('componentId', this.componentId);
      onCallback(callbackId, true, payload.toJSONString());
    }
  }

  private async toggleFullscreen(): Promise<void> {
    try {
      // Get screen dimensions
      const displayInfo = display.getDefaultDisplaySync();
      const density = displayInfo.densityPixels || 1;
      const screenWidth = displayInfo.width / density;
      const screenHeight = displayInfo.height / density;

      // Get NodeController and toggle fullscreen
      const nodeController = getNodeControllerByComponentId(this.componentId);
      if (!nodeController) {
        nativeComponentError(`NodeController not found for: ${this.componentId}`);
        return;
      }

      nodeController.toggleFullscreen(screenWidth, screenHeight);

      // Update system bars
      const context = getContext(this);
      const windowClass = await window.getLastWindow(context);

      if (nodeController.getIsFullscreen()) {
        await windowClass.setWindowSystemBarEnable([]);
      } else {
        await windowClass.setWindowSystemBarEnable(['status', 'navigation']);
      }
    } catch (error) {
      nativeComponentError(`Fullscreen toggle error: ${JSON.stringify(error)}`);
    }
  }

  build() {
    // Use native video player with XComponent
    LxNativeVideoPlayer({
      playerId: this.componentId,
      config: this.buildConfig(),
      onEvent: (event: string, detail: VideoEventDetail) => {
        this.handleEvent(event, detail);
      }
    })
      .width('100%')
      .height('100%')
  }
}

@Builder
function videoBuilder(params: NativeComponentParams) {
  NativeVideoComponent({ params: params })
}

/**
 * Register video builder for same-level rendering
 */
export function registerVideoBuilder(): void {
  registerNativeComponentBuilder('video.native', wrapBuilder(videoBuilder));
}
