/**
 * InputBuilder - Builder for input.native component on Harmony
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

const SEMANTIC_TEXT: string = 'text';
const SEMANTIC_NUMBER: string = 'number';
const SEMANTIC_DIGIT: string = 'digit';

function sanitizeInputValue(value: string, semanticType: string): string {
  if (semanticType === SEMANTIC_NUMBER) {
    return value.replace(/[^0-9]/g, '');
  }
  if (semanticType === SEMANTIC_DIGIT) {
    let output: string = '';
    let hasDot: boolean = false;
    for (let i = 0; i < value.length; i += 1) {
      const ch = value.charAt(i);
      if (ch >= '0' && ch <= '9') {
        output += ch;
      } else if (ch === '.' && !hasDot) {
        output += ch;
        hasDot = true;
      }
    }
    return output;
  }
  return value;
}

function enterKeyTypeFromConfirmType(confirmType: string): EnterKeyType {
  switch (confirmType) {
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

@Component
struct NativeInputComponent {
  @Prop @Watch('onParamsChange') params: NativeComponentParams;
  @State private textValue: string = '';
  @State private placeholderText: string = '';
  @State private isEnabledState: boolean = true;
  @State private maxLength: number = -1;
  @State private placeholderColor: string = DEFAULT_PLACEHOLDER_COLOR;
  @State private enterKeyType: EnterKeyType = EnterKeyType.Done;
  @State private inputType: InputType = InputType.Normal;
  @State private semanticType: string = SEMANTIC_TEXT;
  @State private confirmHold: boolean = false;
  @State private holdKeyboard: boolean = false;
  @State private adjustPosition: boolean = true;
  private componentId: string = '';
  private focusId: string = '';
  private lastFocusedValue: string = '';
  private lastFocusPropValue: boolean = false;
  private lastKeyboardHeight: number = -1;
  private keyboardProbeTimers: number[] = [];
  private inputController: TextInputController = new TextInputController();

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
      this.focusId = `lx-native-input-${componentId}`;
      return this.focusId;
    }
    if (this.focusId.length === 0) {
      this.focusId = 'lx-native-input';
    }
    return this.focusId;
  }

  aboutToAppear(): void {
    this.applyProps();
  }

  aboutToDisappear(): void {
    this.clearKeyboardProbeTimers();
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
      this.focusId = `lx-native-input-${nextComponentId}`;
    } else if (this.focusId.length === 0) {
      this.focusId = 'lx-native-input';
    }
    const props = params.props || new NativeComponentRecord();

    let nextMaxLength = Math.floor(getNumberProp(props, 'maxlength', this.maxLength));
    if (nextMaxLength < 0) {
      nextMaxLength = -1;
    }
    this.maxLength = nextMaxLength;
    this.placeholderColor = resolvePlaceholderColor(props);
    const incomingValue = getStringProp(props, 'value', this.textValue);
    this.textValue = (this.maxLength >= 0 && incomingValue.length > this.maxLength)
      ? incomingValue.substring(0, this.maxLength)
      : incomingValue;
    this.placeholderText = getStringProp(props, 'placeholder', this.placeholderText);
    this.isEnabledState = !getBooleanProp(props, 'disabled', false);

    const confirmType = getStringProp(props, 'confirmType', getStringProp(props, 'confirm-type', 'done')).toLowerCase();
    this.enterKeyType = enterKeyTypeFromConfirmType(confirmType);
    this.confirmHold = getBooleanProp(props, 'confirmHold', getBooleanProp(props, 'confirm-hold', false));
    this.holdKeyboard = getBooleanProp(props, 'holdKeyboard', getBooleanProp(props, 'hold-keyboard', false));
    this.adjustPosition = getBooleanProp(props, 'adjustPosition', getBooleanProp(props, 'adjust-position', true));
    const focus = getBooleanProp(props, 'focus', getBooleanProp(props, 'autoFocus', getBooleanProp(props, 'auto-focus', false)));
    if (!focus && this.lastFocusPropValue) {
      this.stopEditingSafely();
    } else if (focus && !this.lastFocusPropValue) {
      this.requestFocusSafely();
    }
    this.lastFocusPropValue = focus;

    const password = getBooleanProp(props, 'password', false);
    const type = getStringProp(props, 'type', 'text').toLowerCase();
    this.semanticType = SEMANTIC_TEXT;
    if (password || type === 'password' || type === 'safe-password') {
      this.inputType = InputType.Password;
      this.semanticType = SEMANTIC_TEXT;
    } else if (type === 'digit') {
      this.inputType = InputType.NUMBER_DECIMAL;
      this.semanticType = SEMANTIC_DIGIT;
    } else if (type === 'number') {
      this.inputType = InputType.Number;
      this.semanticType = SEMANTIC_NUMBER;
    } else {
      this.inputType = InputType.Normal;
    }
    this.textValue = sanitizeInputValue(this.textValue, this.semanticType);
  }

  private emit(event: string, detail: NativeComponentRecord): void {
    onNativeComponentEventFromBuilder(this.resolveComponentId(), event, detail as NativeComponentValue);
  }

  private makeValueDetail(value: string): NativeComponentRecord {
    const detail = new NativeComponentRecord();
    detail.set('value', value);
    detail.set('cursor', value.length);
    return detail;
  }

  private stopEditingSafely(): void {
    this.inputController.stopEditing();
    setTimeout(() => {
      this.inputController.stopEditing();
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
    TextInput({ text: this.textValue, placeholder: this.placeholderText, controller: this.inputController })
      .id(this.resolveFocusId())
      .focusable(true)
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .border({ width: 0 })
      .padding({ left: 0, right: 0, top: 0, bottom: 0 })
      .enabled(this.isEnabledState)
      .enterKeyType(this.enterKeyType)
      .placeholderColor(this.placeholderColor)
      .maxLength(this.maxLength >= 0 ? this.maxLength : 2147483647)
      .type(this.inputType)
      .onChange((value: string) => {
        let next = sanitizeInputValue(value, this.semanticType);
        if (this.maxLength >= 0 && next.length > this.maxLength) {
          next = next.substring(0, this.maxLength);
        }
        this.textValue = next;
        this.emit('input', this.makeValueDetail(next));
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
        this.emit('confirm', this.makeValueDetail(this.textValue));
        if (!this.confirmHold && !this.holdKeyboard) {
          this.lastFocusPropValue = false;
          this.stopEditingSafely();
        }
      })
  }
}

@Builder
function inputBuilder(params: NativeComponentParams) {
  NativeInputComponent({ params: params });
}

export function registerInputBuilder(): void {
  registerNativeComponentBuilder('input.native', wrapBuilder(inputBuilder));
}
