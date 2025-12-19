/**
 * VideoBuilder - Builder for video component in same-level rendering
 * Uses native XComponent + OH_AVPlayer for direct control
 */
import { display, window } from '@kit.ArkUI';
import { SameLevelParams, registerSameLevelBuilder } from '../SameLevelNodeController';
import { SameLevelRecord, sameLevelError } from '../SameLevelTypes';
import { getNodeControllerByComponentId } from '../SameLevelBridge';
import {
  LxNativeVideoPlayer,
  LxNativeVideoConfig,
  LxMediaQuality
} from '../../media/LxNativeVideoPlayer';

interface QualityProp {
  label?: string;
  url?: string;
}

/**
 * Video component wrapper for same-level rendering
 * Uses native XComponent + OH_AVPlayer for direct control
 */
@Component
struct SameLevelVideoComponent {
  @Prop @Watch('onParamsChange') params: SameLevelParams;
  private eventSink?: (event: SameLevelRecord) => void;
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
      const props = this.params.props;
      this.eventSink = props.get('__eventSink') as ESObject as ((event: SameLevelRecord) => void) | undefined;
    }
  }

  onParamsChange(): void {
    // Handle commands from updateNode
    const command = (this.params as ESObject)?.__command as string;
    if (command === 'pause') {
      // Use static method to pause native player directly
      LxNativeVideoPlayer.pauseStatic(this.componentId);
    } else if (command === 'stop') {
      // Stop playback and reset to initial state (show poster, reset progress)
      LxNativeVideoPlayer.stopStatic(this.componentId);
    } else if (command === 'resume') {
      // Use static method to resume to previous state (only plays if was playing before)
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

    const cornerRadius = props.get('cornerRadius');
    if (typeof cornerRadius === 'number') config.cornerRadius = cornerRadius;

    const objectFit = props.get('objectFit');
    if (typeof objectFit === 'string') config.objectFit = objectFit;

    // Parse qualities
    const qualities = props.get('qualities');
    if (Array.isArray(qualities)) {
      config.qualities = qualities.map((entry: ESObject): LxMediaQuality => {
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

  private handleEvent(event: string, detail: object): void {
    // Handle fullscreen by toggling NodeController state
    if (event === 'fullscreenchange') {
      // Keep native player alive while we rebuild into fullscreen/display mode
      LxNativeVideoPlayer.setKeepAlive(this.componentId, true);
      this.toggleFullscreen();
    }

    // Forward event to JS
    if (this.eventSink) {
      const payload = new SameLevelRecord();
      payload.set('action', 'component.event');
      payload.set('id', this.componentId);
      payload.set('event', event);
      payload.set('detail', detail);
      payload.set('componentId', this.componentId);
      this.eventSink(payload);
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
        sameLevelError(`NodeController not found for: ${this.componentId}`);
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
      sameLevelError(`Fullscreen toggle error: ${JSON.stringify(error)}`);
    }
  }

  build() {
    // Use native video player with XComponent
    LxNativeVideoPlayer({
      playerId: this.componentId,
      config: this.buildConfig(),
      onEvent: (event: string, detail: object) => {
        this.handleEvent(event, detail);
      }
    })
      .width('100%')
      .height('100%')
  }
}

@Builder
function videoBuilder(params: SameLevelParams) {
  SameLevelVideoComponent({ params: params })
}

/**
 * Register video builder for same-level rendering
 */
export function registerVideoBuilder(): void {
  registerSameLevelBuilder('video.native', wrapBuilder(videoBuilder));
}
