/**
 * PickerBuilder - Builder for picker component in same-level rendering
 * Uses LxAppPicker for native picker display
 */
import { NativeComponentParams, registerNativeComponentBuilder } from '../NativeComponentNodeController';
import { NativeComponentRecord } from '../NativeComponentTypes';
import { LxAppPicker, PickerResult } from '../../APIs/LxAppPicker';
import { LxAppDatePicker, LxDatePickerResult } from '../../APIs/LxAppDatePicker';
import { dispatchNativeComponentEventFromBuilder } from '../NativeBridgeInternal';

// Prevent picker from showing multiple times for the same component instance
const showingComponents: Set<string> = new Set();

interface PickerEventDetail {
  index?: number | number[];
  value?: string | string[];
  confirmed?: boolean;
  cancelled?: boolean;
}

/**
 * Picker component wrapper for same-level rendering
 */
@Component
struct NativePickerComponent {
  @Prop @Watch('onParamsChange') params: NativeComponentParams;
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
      // Auto-show picker on mount
      this.showPickerWithProps(props);
    }
  }

  aboutToDisappear(): void {
    if (this.callbackId) {
      LxAppPicker.localCallbacks.delete(this.callbackId);
      LxAppDatePicker.localCallbacks.delete(this.callbackId);
    }
    if (this.componentId) {
      showingComponents.delete(this.componentId);
    }
  }

  onParamsChange(): void {}

  private getStringProp(props: NativeComponentRecord, key: string, defaultValue: string): string {
    const value = props.get(key);
    return typeof value === 'string' ? value : defaultValue;
  }

  private showPickerWithProps(props: NativeComponentRecord): void {
    if (showingComponents.has(this.componentId)) {
      return;
    }
    showingComponents.add(this.componentId);

    const mode = this.getStringProp(props, 'mode', 'selector');

    NativePickerComponent.nextId++;
    this.callbackId = `picker-${this.componentId}-${NativePickerComponent.nextId}`;

    const componentId = this.componentId;
    const callback = (success: boolean, resultJSON: string) => {
      const isTerminal = this.handlePickerCallback(success, resultJSON, mode);
      if (isTerminal) {
        setTimeout(() => {
          showingComponents.delete(componentId);
        }, 300);
      }
    };

    if (mode === 'date' || mode === 'time') {
      LxAppDatePicker.localCallbacks.set(this.callbackId, callback);

      const fields = this.getStringProp(props, 'fields', 'day');
      const valueRaw = props.get('value');
      let value = '';
      if (typeof valueRaw === 'string') {
        value = valueRaw;
      } else if (valueRaw !== undefined) {
        value = JSON.stringify(valueRaw);
      }

      LxAppDatePicker.showDatePicker(
        mode,
        fields,
        value,
        this.getStringProp(props, 'start', ''),
        this.getStringProp(props, 'end', ''),
        this.getStringProp(props, 'cancelText', ''),
        this.getStringProp(props, 'cancelButtonColor', ''),
        this.getStringProp(props, 'cancelTextColor', ''),
        this.getStringProp(props, 'confirmText', ''),
        this.getStringProp(props, 'confirmButtonColor', ''),
        this.getStringProp(props, 'confirmTextColor', ''),
        this.callbackId
      );
    } else {
      LxAppPicker.localCallbacks.set(this.callbackId, callback);

      const columnsRaw = props.get('columns');
      let columnsJSON: string;
      if (typeof columnsRaw === 'string') {
        columnsJSON = columnsRaw;
      } else if (columnsRaw !== undefined) {
        columnsJSON = JSON.stringify(columnsRaw);
      } else {
        columnsJSON = '[]';
      }

      LxAppPicker.showPicker(
        columnsJSON,
        this.getStringProp(props, 'cancelText', ''),
        this.getStringProp(props, 'cancelButtonColor', ''),
        this.getStringProp(props, 'cancelTextColor', ''),
        this.getStringProp(props, 'confirmText', ''),
        this.getStringProp(props, 'confirmButtonColor', ''),
        this.getStringProp(props, 'confirmTextColor', ''),
        this.callbackId
      );
    }
  }

  private handlePickerCallback(success: boolean, resultJSON: string, mode: string): boolean {
    if (!success) {
      this.sendEvent('change', { cancelled: true });
      return true;
    }

    try {
      const detail: PickerEventDetail = {};
      let isTerminal = false;

      if (mode === 'date' || mode === 'time') {
        const result: LxDatePickerResult = JSON.parse(resultJSON);
        if (result.value !== undefined) {
          detail.value = result.value;
        }
        if (result.confirm === true) {
          detail.confirmed = true;
          isTerminal = true;
        } else if (result.cancel === true) {
          detail.cancelled = true;
          isTerminal = true;
        }
      } else {
        const result: PickerResult = JSON.parse(resultJSON);
        if (result.index !== undefined) {
          detail.index = result.index;
        }
        if (result.confirm === true) {
          detail.confirmed = true;
          isTerminal = true;
        } else if (result.cancel === true) {
          detail.cancelled = true;
          isTerminal = true;
        }
      }

      this.sendEvent(isTerminal ? 'change' : 'scroll', detail);
      return isTerminal;
    } catch (error) {
      // Ignore parse errors
    }
    return false;
  }

  private sendEvent(event: string, detail: PickerEventDetail): void {
    dispatchNativeComponentEventFromBuilder(this.componentId, event, detail);
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
function pickerBuilder(params: NativeComponentParams) {
  NativePickerComponent({ params: params })
}

/**
 * Register picker builder for same-level rendering
 */
export function registerPickerBuilder(): void {
  registerNativeComponentBuilder('picker.native', wrapBuilder(pickerBuilder));
}
