/**
 * PickerBuilder - Builder for picker component in same-level rendering
 * Uses LxAppPicker for native picker display
 */
import { SameLevelParams, registerSameLevelBuilder } from '../SameLevelNodeController';
import { SameLevelRecord } from '../SameLevelTypes';
import { LxAppPicker, PickerResult } from '../../APIs/LxAppPicker';

// Prevent picker from showing multiple times for the same component instance
const showingComponents: Set<string> = new Set();

interface PickerEventDetail {
  index?: number | number[];
  confirmed?: boolean;
  cancelled?: boolean;
}

/**
 * Picker component wrapper for same-level rendering
 */
@Component
struct SameLevelPickerComponent {
  @Prop @Watch('onParamsChange') params: SameLevelParams;
  private eventSink?: (event: SameLevelRecord) => void;
  private componentId: string = '';
  private initialized: boolean = false;
  private callbackId: string = '';
  private static nextId: number = 1;

  aboutToAppear(): void {
    if (this.initialized) {
      return;
    }
    this.initialized = true;

    if (this.params) {
      this.componentId = this.params.componentId || '';
      const props = this.params.props;
      const eventSinkValue = props.get('__eventSink');
      if (typeof eventSinkValue === 'function') {
        this.eventSink = eventSinkValue as (event: SameLevelRecord) => void;
      }

      // Auto-show picker on mount
      this.showPickerWithProps(props);
    }
  }

  aboutToDisappear(): void {
    // Cleanup callback and tracking flag on unmount
    if (this.callbackId) {
      LxAppPicker.localCallbacks.delete(this.callbackId);
    }
    if (this.componentId) {
      showingComponents.delete(this.componentId);
    }
  }

  onParamsChange(): void {
    // No runtime updates needed for picker
  }

  private getStringProp(props: SameLevelRecord, key: string, defaultValue: string): string {
    const value = props.get(key);
    return typeof value === 'string' ? value : defaultValue;
  }

  private showPickerWithProps(props: SameLevelRecord): void {
    // Prevent duplicate picker show for same component
    if (showingComponents.has(this.componentId)) {
      return;
    }
    showingComponents.add(this.componentId);

    const columnsRaw = props.get('columns');
    let columnsJSON: string;
    if (typeof columnsRaw === 'string') {
      columnsJSON = columnsRaw;
    } else if (columnsRaw !== undefined) {
      columnsJSON = JSON.stringify(columnsRaw);
    } else {
      columnsJSON = '[]';
    }

    // Generate unique callback ID
    SameLevelPickerComponent.nextId++;
    this.callbackId = `picker-${this.componentId}-${SameLevelPickerComponent.nextId}`;

    // Register callback - LxAppPicker manages callback lifecycle
    const componentId = this.componentId;
    LxAppPicker.localCallbacks.set(this.callbackId, (success: boolean, resultJSON: string) => {
      this.handlePickerCallback(success, resultJSON);

      // Clean up showingComponents on terminal events with delay
      // Delay ensures picker UI is fully hidden before allowing re-show
      const isTerminal = resultJSON.includes('"confirm"') || resultJSON.includes('"cancel"');
      if (isTerminal) {
        setTimeout(() => {
          showingComponents.delete(componentId);
        }, 300);
      }
    });

    LxAppPicker.showPicker(
      columnsJSON,
      this.getStringProp(props, 'cancelText', 'Cancel'),
      this.getStringProp(props, 'cancelButtonColor', ''),
      this.getStringProp(props, 'cancelTextColor', ''),
      this.getStringProp(props, 'confirmText', 'OK'),
      this.getStringProp(props, 'confirmButtonColor', ''),
      this.getStringProp(props, 'confirmTextColor', ''),
      this.callbackId
    );
  }

  private handlePickerCallback(success: boolean, resultJSON: string): void {
    if (!success) {
      this.sendEvent('change', { cancelled: true });
      return;
    }

    try {
      const result: PickerResult = JSON.parse(resultJSON);
      const detail: PickerEventDetail = {};

      if (result.index !== undefined) {
        detail.index = result.index;
      }
      if (result.confirm === true) {
        detail.confirmed = true;
      } else if (result.cancel === true) {
        detail.cancelled = true;
      }

      this.sendEvent('change', detail);
    } catch (error) {
      // Ignore parse errors
    }
  }

  private sendEvent(event: string, detail: PickerEventDetail): void {
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

  build() {
    // Picker is a modal overlay, no inline content needed
    Column()
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
  }
}

@Builder
function pickerBuilder(params: SameLevelParams) {
  SameLevelPickerComponent({ params: params })
}

/**
 * Register picker builder for same-level rendering
 */
export function registerPickerBuilder(): void {
  registerSameLevelBuilder('picker.native', wrapBuilder(pickerBuilder));
}
