/**
 * VideoBuilder - Builder for video component in same-level rendering
 * Uses native XComponent + OH_AVPlayer for direct control
 */
import { display, window } from '@kit.ArkUI';
import { SameLevelParams, registerSameLevelBuilder } from '../SameLevelNodeController';
import { sameLevelError } from '../SameLevelTypes';
import { getNodeControllerByComponentId } from '../SameLevelBridge';
import {
  LxNativeVideoPlayer,
  LxNativeVideoConfig,
  LxMediaQuality
} from '../../media/LxNativeVideoPlayer';

type QualityProp = Record<string, Object>;

/**
 * Video component wrapper for same-level rendering
 * Uses native XComponent + OH_AVPlayer for direct control
 */
@Component
struct SameLevelVideoComponent {
  @Prop @Watch('onParamsChange') params: SameLevelParams;
  private eventSink?: (event: Record<string, Object>) => void;
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
      if (props) {
        this.eventSink = props['__eventSink'] as ((event: Record<string, Object>) => void) | undefined;
      }
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
    if (props['src']) {
      config.src = props['src'] as string;
    }

    if (props['poster']) config.poster = props['poster'] as string;
    if (props['autoplay'] !== undefined) config.autoplay = props['autoplay'] as boolean;
    if (props['loop'] !== undefined) config.loop = props['loop'] as boolean;
    if (props['muted'] !== undefined) config.muted = props['muted'] as boolean;
    if (props['volume'] !== undefined) config.volume = props['volume'] as number;
    if (props['controls'] !== undefined) config.controls = props['controls'] as boolean;
    if (props['cornerRadius'] !== undefined) {
      const radiusVal = props['cornerRadius'];
      const parsed = typeof radiusVal === 'string' ? parseFloat(radiusVal) : Number(radiusVal);
      config.cornerRadius = isNaN(parsed) ? 0 : parsed;
    }
    if (props['objectFit']) config.objectFit = props['objectFit'] as string;

    // Parse qualities
    if (props['qualities']) {
      const qualitiesList = props['qualities'] as Array<QualityProp>;
      config.qualities = qualitiesList.map((entry: QualityProp): LxMediaQuality => {
        const quality = new LxMediaQuality();
        const label = entry['label'];
        const url = entry['url'];
        if (typeof label === 'string') quality.label = label;
        if (typeof url === 'string') quality.url = url;
        return quality;
      }).filter((q: LxMediaQuality): boolean => q.label.length > 0);
    }

    // Parse playbackRates (first is default)
    if (props['playbackRates']) {
      const ratesList = props['playbackRates'] as Array<number>;
      config.playbackRates = ratesList.filter((r: number): boolean => !isNaN(r));
    }

    // Cache the valid config
    this.cachedConfig = config;
    return config;
  }

  private handleEvent(event: string, detail: Object): void {
    // Handle fullscreen by toggling NodeController state
    if (event === 'fullscreenchange') {
      // Keep native player alive while we rebuild into fullscreen/display mode
      LxNativeVideoPlayer.setKeepAlive(this.componentId, true);
      this.toggleFullscreen();
    }

    // Forward event to JS
    if (this.eventSink) {
      const payload: Record<string, Object> = new Object() as Record<string, Object>;
      payload['action'] = 'component.event';
      payload['id'] = this.componentId;
      payload['event'] = event;
      payload['detail'] = detail;
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
      onEvent: (event: string, detail: Object) => {
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
