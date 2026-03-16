/**
 * TextareaBuilder - Builder for textarea.native component on Harmony
 */
import { NativeComponentParams, registerNativeComponentBuilder } from '../NativeComponentNodeController';
import { NativeComponentRecord, NativeComponentValue } from '../NativeComponentTypes';
import { onNativeComponentEventFromBuilder } from '../NativeBridgeInternal';
import {
  DEFAULT_PLACEHOLDER_COLOR,
  getStringProp,
  getBooleanProp,
  getNumberProp,
  resolvePlaceholderColor,
  readKeyboardHeightVp,
} from './BuilderUtils';

function enterKeyTypeFromConfirmType(confirmType: string): EnterKeyType {
  switch (confirmType) {
    case 'return':
      return EnterKeyType.NEW_LINE;
    case 'go':
      return EnterKeyType.Go;
    case 'send':
      return EnterKeyType.Send;
    case 'search':
      return EnterKeyType.Search;
    case 'next':
      return EnterKeyType.Next;
    default:
      return EnterKeyType.Done;
  }
}

function normalizeConfirmType(confirmType: string): string {
  const normalized = confirmType.trim().toLowerCase();
  switch (normalized) {
    case 'send':
    case 'search':
    case 'next':
    case 'go':
    case 'done':
    case 'return':
      return normalized;
    default:
      return 'return';
  }
}

class TextareaLineMetrics {
  lineCount: number = 1;
  height: number = 24;
}

@Component
struct NativeTextareaComponent {
  @Prop @Watch('onParamsChange') params: NativeComponentParams;
  @State private textValue: string = '';
  @State private placeholderText: string = '';
  @State private isEnabledState: boolean = true;
  @State private maxLength: number = -1;
  @State private placeholderColor: string = DEFAULT_PLACEHOLDER_COLOR;
  @State private enterKeyType: EnterKeyType = EnterKeyType.Done;
  @State private confirmType: string = 'return';
  @State private confirmHold: boolean = false;
  @State private holdKeyboard: boolean = false;
  @State private adjustPosition: boolean = true;
  @State private autoHeight: boolean = false;
  @State private disableDefaultPadding: boolean = false;
  private componentId: string = '';
  private focusId: string = '';
  private lastLineCount: number = 0;
  private lastContentHeight: number = 0;
  private lastFocusedValue: string = '';
  private lastInputValue: string = '';
  private lastFocusPropValue: boolean = false;
  private lastKeyboardHeight: number = -1;
  private componentHeightVp: number = 0;
  private controllerBaselineLineCount: number = 0;
  private controllerBaselineHeightPx: number = 0;
  private keyboardProbeTimers: number[] = [];
  private lineMetricsProbeTimers: number[] = [];
  private textareaController: TextAreaController = new TextAreaController();

  private resolveComponentId(): string {
    const fromParams = this.params?.componentId || '';
    if (fromParams.length > 0) {
      this.componentId = fromParams;
      return fromParams;
    }
    return this.componentId;
  }

  private resolveFocusId(): string {
    const componentId = this.resolveComponentId();
    if (componentId.length > 0) {
      this.focusId = `lx-native-textarea-${componentId}`;
      return this.focusId;
    }
    if (this.focusId.length === 0) {
      this.focusId = 'lx-native-textarea';
    }
    return this.focusId;
  }

  aboutToAppear(): void {
    this.applyProps();
  }

  aboutToDisappear(): void {
    this.clearKeyboardProbeTimers();
    this.clearLineMetricsProbeTimers();
    this.lastKeyboardHeight = -1;
  }

  onParamsChange(): void {
    this.applyProps();
  }

  private applyProps(): void {
    const params = this.params;
    if (!params) {
      return;
    }
    const command = (params as ESObject)?.__command as string;
    if (typeof command === 'string' && command.length > 0) {
      return;
    }
    const nextComponentId = params.componentId || '';
    if (nextComponentId.length > 0) {
      this.componentId = nextComponentId;
      this.focusId = `lx-native-textarea-${nextComponentId}`;
    } else if (this.focusId.length === 0) {
      this.focusId = 'lx-native-textarea';
    }
    const nextHeightVp = Number(params.height);
    if (Number.isFinite(nextHeightVp) && nextHeightVp > 0) {
      this.componentHeightVp = nextHeightVp;
    }
    const props = params.props || new NativeComponentRecord();

    let nextMaxLength = Math.floor(getNumberProp(props, 'maxlength', this.maxLength));
    if (nextMaxLength < 0) {
      nextMaxLength = -1;
    }
    this.maxLength = nextMaxLength;
    this.placeholderColor = resolvePlaceholderColor(props);
    if (props.has('value')) {
      const incomingValue = getStringProp(props, 'value', this.textValue);
      const normalizedValue = (this.maxLength >= 0 && incomingValue.length > this.maxLength)
        ? incomingValue.substring(0, this.maxLength)
        : incomingValue;
      this.textValue = normalizedValue;
      this.lastInputValue = normalizedValue;
    } else if (this.maxLength >= 0 && this.textValue.length > this.maxLength) {
      this.textValue = this.textValue.substring(0, this.maxLength);
    }
    this.placeholderText = getStringProp(props, 'placeholder', this.placeholderText);
    this.isEnabledState = !getBooleanProp(props, 'disabled', false);

    const confirmType = normalizeConfirmType(
      getStringProp(props, 'confirmType', getStringProp(props, 'confirm-type', 'return'))
    );
    this.confirmType = confirmType;
    this.enterKeyType = enterKeyTypeFromConfirmType(confirmType);
    this.confirmHold = getBooleanProp(props, 'confirmHold', getBooleanProp(props, 'confirm-hold', false));
    this.holdKeyboard = getBooleanProp(props, 'holdKeyboard', getBooleanProp(props, 'hold-keyboard', false));
    this.adjustPosition = getBooleanProp(props, 'adjustPosition', getBooleanProp(props, 'adjust-position', true));
    this.autoHeight = getBooleanProp(props, 'autoHeight', getBooleanProp(props, 'auto-height', false));
    this.disableDefaultPadding = getBooleanProp(
      props,
      'disableDefaultPadding',
      getBooleanProp(props, 'disable-default-padding', false)
    );
    const focus = getBooleanProp(props, 'focus', getBooleanProp(props, 'autoFocus', getBooleanProp(props, 'auto-focus', false)));
    if (!focus && this.lastFocusPropValue) {
      this.stopEditingSafely();
    } else if (focus && !this.lastFocusPropValue) {
      this.requestFocusSafely();
    }
    this.lastFocusPropValue = focus;
    this.emitLineChangeIfNeeded(this.textValue);
    this.scheduleLineMetricsProbe(this.textValue);
  }

  private emit(event: string, detail: NativeComponentRecord): void {
    const componentId = this.resolveComponentId();
    onNativeComponentEventFromBuilder(componentId, event, detail as NativeComponentValue);
  }

  private makeValueDetail(value: string): NativeComponentRecord {
    const detail = new NativeComponentRecord();
    const cursor = value.length;
    detail.set('value', value);
    detail.set('cursor', cursor);
    detail.set('selectionStart', cursor);
    detail.set('selectionEnd', cursor);
    return detail;
  }

  private makeLinechangeDetail(lineCount: number, height: number): NativeComponentRecord {
    const detail = new NativeComponentRecord();
    const estimatedHeight = Math.max(24, Math.round(height));
    detail.set('lineCount', lineCount);
    detail.set('height', estimatedHeight);
    detail.set('heightRpx', estimatedHeight);
    return detail;
  }

  private lineCountWithHardBreaks(text: string): number {
    if (text.length === 0) {
      return 1;
    }
    return text.split('\n').length;
  }

  private readControllerLineCount(): number {
    try {
      const raw = Number(this.textareaController.getTextContentLineCount());
      if (Number.isFinite(raw) && raw > 0) {
        return Math.floor(raw);
      }
    } catch (_error) {
      // Ignore controller read failures and use fallback estimates.
    }
    return 0;
  }

  private readControllerHeightPx(): number {
    try {
      const rect = this.textareaController.getTextContentRect();
      const heightPx = Number(rect.height);
      if (!Number.isFinite(heightPx) || heightPx <= 0) {
        return 0;
      }
      if (Number.isFinite(heightPx) && heightPx > 0) {
        return heightPx;
      }
    } catch (_error) {
      // Ignore controller read failures and use fallback estimates.
    }
    return 0;
  }

  private computeLineMetrics(value: string): TextareaLineMetrics {
    const metrics = new TextareaLineMetrics();
    const hardBreakLineCount = this.lineCountWithHardBreaks(value);
    const estimatedLineHeight = 22;
    if (this.autoHeight) {
      // For auto-height, prioritize semantic line count from actual hard breaks.
      // Harmony TextArea controller metrics can include viewport blank rows,
      // which causes severe over-estimation (e.g. 2-3 lines reported as 7+).
      const lineCount = Math.max(1, hardBreakLineCount);
      const autoHeightLineHeight = 20;
      const verticalPadding = this.disableDefaultPadding ? 0 : 3;
      metrics.lineCount = lineCount;
      metrics.height = Math.max(24, lineCount * autoHeightLineHeight + verticalPadding);
      return metrics;
    }
    let lineCount = hardBreakLineCount;
    const controllerLineCount = this.readControllerLineCount();
    if (controllerLineCount > 0) {
      if (this.controllerBaselineLineCount <= 0) {
        this.controllerBaselineLineCount = controllerLineCount;
      } else if (
        hardBreakLineCount <= 1 &&
        controllerLineCount < this.controllerBaselineLineCount
      ) {
        this.controllerBaselineLineCount = controllerLineCount;
      }
      const normalizedLineCount = Math.max(
        1,
        controllerLineCount - this.controllerBaselineLineCount + 1
      );
      if (normalizedLineCount > lineCount) {
        lineCount = normalizedLineCount;
      }
    }

    let height = Math.max(24, lineCount * estimatedLineHeight);
    const controllerHeight = this.readControllerHeightPx();
    if (controllerHeight > 0) {
      if (this.controllerBaselineHeightPx <= 0) {
        this.controllerBaselineHeightPx = controllerHeight;
      } else if (
        hardBreakLineCount <= 1 &&
        controllerHeight < this.controllerBaselineHeightPx
      ) {
        this.controllerBaselineHeightPx = controllerHeight;
      }
      const baseSingleLineHeight = this.disableDefaultPadding ? estimatedLineHeight : (estimatedLineHeight + 4);
      const normalizedContentHeight = this.controllerBaselineHeightPx > 0
        ? Math.max(baseSingleLineHeight, controllerHeight - this.controllerBaselineHeightPx + baseSingleLineHeight)
        : controllerHeight;
      const verticalPadding = this.disableDefaultPadding ? 0 : 4;
      const contentHeight = Math.max(0, normalizedContentHeight);
      const estimatedFromHeight = Math.max(
        1,
        Math.ceil(Math.max(1, contentHeight - verticalPadding) / estimatedLineHeight)
      );
      if (estimatedFromHeight > lineCount) {
        lineCount = estimatedFromHeight;
      }
      height = Math.max(height, Math.round(contentHeight + verticalPadding));
    }
    metrics.lineCount = lineCount;
    metrics.height = height;
    return metrics;
  }

  private emitLineChangeIfNeeded(value: string): void {
    const metrics = this.computeLineMetrics(value);
    if (metrics.lineCount === this.lastLineCount && Math.abs(metrics.height - this.lastContentHeight) < 1) {
      return;
    }
    this.lastLineCount = metrics.lineCount;
    this.lastContentHeight = metrics.height;
    this.emit('linechange', this.makeLinechangeDetail(metrics.lineCount, metrics.height));
  }

  private handleTextChanged(rawValue: string): void {
    let next = rawValue;
    if (this.maxLength >= 0 && next.length > this.maxLength) {
      next = next.substring(0, this.maxLength);
    }
    this.textValue = next;
    if (next !== this.lastInputValue) {
      this.lastInputValue = next;
      this.emit('input', this.makeValueDetail(next));
    }
    this.emitLineChangeIfNeeded(next);
    this.scheduleLineMetricsProbe(next);
  }

  private clearLineMetricsProbeTimers(): void {
    if (this.lineMetricsProbeTimers.length === 0) {
      return;
    }
    this.lineMetricsProbeTimers.forEach((timerId: number) => {
      clearTimeout(timerId);
    });
    this.lineMetricsProbeTimers = [];
  }

  private scheduleLineMetricsProbe(expectedValue: string): void {
    this.clearLineMetricsProbeTimers();
    const delays = [0, 16, 48, 96];
    delays.forEach((delay: number) => {
      const timerId = setTimeout(() => {
        if (expectedValue !== this.textValue) {
          return;
        }
        this.emitLineChangeIfNeeded(this.textValue);
      }, delay);
      this.lineMetricsProbeTimers.push(timerId);
    });
  }

  private stopEditingSafely(): void {
    this.textareaController.stopEditing();
    setTimeout(() => {
      this.textareaController.stopEditing();
    }, 16);
  }

  private requestFocusSafely(): void {
    const targetKey = this.resolveFocusId();
    setTimeout(() => {
      if (!this.lastFocusPropValue) {
        return;
      }
      focusControl.requestFocus(targetKey);
    }, 0);
  }

  private clearKeyboardProbeTimers(): void {
    if (this.keyboardProbeTimers.length === 0) {
      return;
    }
    this.keyboardProbeTimers.forEach((timerId: number) => {
      clearTimeout(timerId);
    });
    this.keyboardProbeTimers = [];
  }

  private emitKeyboardHeightChange(height: number): void {
    const normalized = Math.max(0, Math.round(height));
    if (normalized === this.lastKeyboardHeight) {
      return;
    }
    this.lastKeyboardHeight = normalized;
    const detail = new NativeComponentRecord();
    detail.set('height', normalized);
    detail.set('duration', 0);
    this.emit('keyboardheightchange', detail);
  }

  private async readKeyboardHeightVp(): Promise<number> {
    return readKeyboardHeightVp(this.getUIContext());
  }

  private scheduleKeyboardProbe(): void {
    this.clearKeyboardProbeTimers();
    if (!this.adjustPosition) {
      this.emitKeyboardHeightChange(0);
      return;
    }
    const delays = [0, 40, 100, 180, 260, 360];
    delays.forEach((delay: number) => {
      const timerId = setTimeout(async () => {
        const height = await this.readKeyboardHeightVp();
        this.emitKeyboardHeightChange(height);
      }, delay);
      this.keyboardProbeTimers.push(timerId);
    });
  }

  build() {
    TextArea({ text: this.textValue, placeholder: this.placeholderText, controller: this.textareaController })
      .id(this.resolveFocusId())
      .focusable(true)
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .border({ width: 0 })
      .padding(this.disableDefaultPadding
        ? { left: 0, right: 0, top: 0, bottom: 0 }
        : (this.autoHeight
          ? { left: 4, right: 4, top: 4, bottom: 0 }
          : { left: 4, right: 4, top: 2, bottom: 2 }))
      .enabled(this.isEnabledState)
      .enterKeyType(this.enterKeyType)
      .placeholderColor(this.placeholderColor)
      .barState(this.autoHeight ? BarState.Off : BarState.Auto)
      .maxLength(this.maxLength >= 0 ? this.maxLength : 2147483647)
      .onChange((value: string) => {
        this.handleTextChanged(value);
      })
      .onContentScroll((_totalOffsetX: number, totalOffsetY: number) => {
        if (!this.autoHeight) {
          return;
        }
        this.scheduleLineMetricsProbe(this.textValue);
        const overflow = Math.max(0, Math.round(totalOffsetY));
        if (overflow <= 0) {
          return;
        }
        const baselineHeight = Math.max(24, this.lastContentHeight);
        const nextHeight = Math.max(baselineHeight, Math.round(baselineHeight + overflow));
        if (nextHeight <= this.lastContentHeight) {
          return;
        }
        this.lastContentHeight = nextHeight;
        this.lastLineCount = Math.max(this.lastLineCount, Math.round(nextHeight / 22));
        this.emit('linechange', this.makeLinechangeDetail(this.lastLineCount, nextHeight));
      })
      .onFocus(() => {
        this.lastFocusedValue = this.textValue;
        this.scheduleKeyboardProbe();
        this.emit('focus', this.makeValueDetail(this.textValue));
      })
      .onBlur(() => {
        this.clearKeyboardProbeTimers();
        this.emitKeyboardHeightChange(0);
        this.lastFocusPropValue = false;
        this.emit('blur', this.makeValueDetail(this.textValue));
        if (this.lastFocusedValue !== this.textValue) {
          this.emit('change', this.makeValueDetail(this.textValue));
        }
      })
      .onSubmit((_enterKey: EnterKeyType) => {
        if (this.confirmType === 'return') {
          return;
        }
        this.emit('confirm', this.makeValueDetail(this.textValue));
        if (!this.confirmHold && !this.holdKeyboard) {
          this.lastFocusPropValue = false;
          this.stopEditingSafely();
        }
      })
  }
}

@Builder
function textareaBuilder(params: NativeComponentParams) {
  NativeTextareaComponent({ params: params });
}

export function registerTextareaBuilder(): void {
  registerNativeComponentBuilder('textarea.native', wrapBuilder(textareaBuilder));
}
