/**
 * Kind-specific ArkUI nodes for the inline native island.
 * Cover/Button stay recipes on the wire (view/tappable); hosts factory
 * only root/view/text/tappable/slider plus video.native.
 */
import { NativeComponentParams, registerNativeComponentBuilder } from '../NativeComponentNodeController';
import { NativeComponentRecord } from '../NativeComponentTypes';
import { onNativeComponentEventFromBuilder } from '../NativeBridgeInternal';
import { getBooleanProp, getNumberProp, getStringProp } from './BuilderUtils';

@Component
struct IslandViewNode {
  @Prop params: NativeComponentParams;

  build() {
    Stack() {
      Column()
        .width('100%')
        .height('100%')
        .linearGradient(this.scrimGradient())
    }
    .width('100%')
    .height('100%')
    .backgroundColor(this.backgroundColor())
    .borderRadius(this.styleNumber('borderRadius', 0))
    .opacity(Math.max(0, Math.min(1, this.styleNumber('opacity', 1))))
    .hitTestBehavior(this.hitMode())
    .accessibilityText(this.ariaLabel())
    .accessibilityDescription(this.ariaDescription())
  }

  private scrimGradient(): { angle: number; colors: Array<[string, number]> } {
    const props = this.params.props;
    const paint = props?.get('scrimPaint');
    let scrim = 'none';
    let opacity = 0.6;
    if (paint && typeof paint === 'object') {
      const record = paint as Record<string, Object>;
      const name = record['scrim'];
      if (typeof name === 'string') {
        scrim = name;
      }
      const rawOpacity = record['opacity'];
      if (typeof rawOpacity === 'number') {
        opacity = rawOpacity;
      }
    }
    const color = `#${Math.round(opacity * 255).toString(16).padStart(2, '0')}000000`;
    if (scrim === 'top') {
      return { angle: 180, colors: [[color, 0], ['#00000000', 1]] };
    }
    if (scrim === 'bottom') {
      return { angle: 180, colors: [['#00000000', 0], [color, 1]] };
    }
    if (scrim === 'full') {
      return { angle: 180, colors: [[color, 0], [color, 1]] };
    }
    return { angle: 180, colors: [['#00000000', 0], ['#00000000', 1]] };
  }

  private hitMode(): HitTestMode {
    const mode = getStringProp(this.params.props, 'pointerEvents', 'box-none');
    if (mode === 'none' || mode === 'box-none') {
      return HitTestMode.None;
    }
    return HitTestMode.Default;
  }

  private styleRecord(): Record<string, Object> | undefined {
    const value = this.params.props?.get('nativeStyle');
    return value && typeof value === 'object' ? value as Record<string, Object> : undefined;
  }

  private styleNumber(key: string, fallback: number): number {
    const record = this.styleRecord();
    const value = record ? record[key] : undefined;
    return typeof value === 'number' ? value : fallback;
  }

  private backgroundColor(): string {
    const record = this.styleRecord();
    const value = record ? record['backgroundColor'] : undefined;
    return typeof value === 'string' && value.length > 0 ? value : '#00000000';
  }

  private ariaLabel(): string {
    return getStringProp(this.params.props, 'aria-label', getStringProp(this.params.props, 'ariaLabel', ''));
  }

  private ariaDescription(): string {
    return getStringProp(this.params.props, 'aria-description', getStringProp(this.params.props, 'ariaDescription', ''));
  }
}

@Component
struct IslandTextNode {
  @Prop params: NativeComponentParams;

  build() {
    Text(getStringProp(this.params.props, 'text', ''))
      .fontColor(this.styleString('color', '#FFFFFF'))
      .fontSize(this.styleNumber('fontSize', 12))
      .fontWeight(this.fontWeight())
      .maxLines(Math.max(1, Math.round(getNumberProp(this.params.props, 'maxLines', 1000))))
      .textOverflow({ overflow: TextOverflow.Ellipsis })
      .textAlign(this.textAlign())
      .width('100%')
      .height('100%')
      .hitTestBehavior(HitTestMode.None)
      .accessibilityText(this.ariaLabel())
      .accessibilityDescription(this.ariaDescription())
      .opacity(this.opacity())
  }

  private styleRecord(): Record<string, Object> | undefined {
    const value = this.params.props?.get('nativeStyle');
    return value && typeof value === 'object' ? value as Record<string, Object> : undefined;
  }

  private styleString(key: string, fallback: string): string {
    const direct = this.params.props?.get(key);
    if (typeof direct === 'string' && direct.length > 0) {
      return direct;
    }
    const record = this.styleRecord();
    const value = record ? record[key] : undefined;
    return typeof value === 'string' && value.length > 0 ? value : fallback;
  }

  private styleNumber(key: string, fallback: number): number {
    const direct = this.params.props?.get(key);
    if (typeof direct === 'number') {
      return direct;
    }
    const record = this.styleRecord();
    const value = record ? record[key] : undefined;
    return typeof value === 'number' ? value : fallback;
  }

  private fontWeight(): FontWeight {
    const value = this.params.props?.get('fontWeight');
    if ((typeof value === 'number' && value >= 600) || value === 'bold' || value === 'bolder') {
      return FontWeight.Bold;
    }
    return FontWeight.Normal;
  }

  private textAlign(): TextAlign {
    const value = getStringProp(this.params.props, 'textAlign', 'start');
    if (value === 'center') return TextAlign.Center;
    if (value === 'end') return TextAlign.End;
    return TextAlign.Start;
  }

  private ariaLabel(): string {
    return getStringProp(this.params.props, 'aria-label', getStringProp(this.params.props, 'ariaLabel', ''));
  }

  private ariaDescription(): string {
    return getStringProp(this.params.props, 'aria-description', getStringProp(this.params.props, 'ariaDescription', ''));
  }

  private opacity(): number {
    return Math.max(0, Math.min(1, this.styleNumber('opacity', 1)));
  }
}

@Component
struct IslandTappableNode {
  @Prop params: NativeComponentParams;

  build() {
    Button(this.label())
      .width('100%')
      .height('100%')
      .backgroundColor(this.backgroundColor())
      .fontColor(this.foregroundColor())
      .fontSize(getStringProp(this.params.props, 'size', 'regular') === 'compact' ? 12 : 14)
      .fontWeight(FontWeight.Bold)
      .enabled(this.enabled())
      .hitTestBehavior(this.hitMode())
      .accessibilityText(this.ariaLabel())
      .accessibilityDescription(this.ariaDescription())
      .onClick(() => {
        if (!this.enabled()) return;
        onNativeComponentEventFromBuilder(this.params.componentId, 'press', { source: 'pointer' });
      })
      .onFocus(() => {
        onNativeComponentEventFromBuilder(this.params.componentId, 'focus', { source: 'keyboard' });
      })
      .onBlur(() => {
        onNativeComponentEventFromBuilder(this.params.componentId, 'blur', { source: 'keyboard' });
      })
      .onHover((hovered: boolean) => {
        onNativeComponentEventFromBuilder(
          this.params.componentId,
          hovered ? 'pointerenter' : 'pointerleave',
          { source: 'pointer' }
        );
      })
  }

  private label(): string {
    if (getBooleanProp(this.params.props, 'loading', false)) {
      return '…';
    }
    const props = this.params.props;
    const content = props?.get('content');
    if (content && typeof content === 'object') {
      const record = content as Record<string, Object>;
      const text = record['text'];
      if (typeof text === 'string' && text.length > 0) {
        return text;
      }
      const icon = record['icon'];
      if (icon && typeof icon === 'object') {
        const name = (icon as Record<string, Object>)['name'];
        if (typeof name === 'string') {
          return name;
        }
      }
    }
    return getStringProp(props, 'label', getStringProp(props, 'icon', ''));
  }

  private enabled(): boolean {
    return !getBooleanProp(this.params.props, 'disabled', false) &&
      !getBooleanProp(this.params.props, 'loading', false);
  }

  private hitMode(): HitTestMode {
    const mode = getStringProp(this.params.props, 'pointerEvents', 'auto');
    return mode === 'none' || mode === 'box-none' ? HitTestMode.None : HitTestMode.Default;
  }

  private ariaLabel(): string {
    return getStringProp(
      this.params.props,
      'aria-label',
      getStringProp(this.params.props, 'ariaLabel', this.label())
    );
  }

  private ariaDescription(): string {
    return getStringProp(this.params.props, 'aria-description', getStringProp(this.params.props, 'ariaDescription', ''));
  }

  private backgroundColor(): string {
    const emphasis = getStringProp(this.params.props, 'emphasis', 'secondary');
    if (emphasis === 'quiet') return '#00000000';
    if (!this.enabled()) return '#FF9CA3AF';
    const intent = getStringProp(this.params.props, 'intent', 'neutral');
    if (intent === 'accent') return emphasis === 'secondary' ? '#502563EB' : '#FF2563EB';
    if (intent === 'destructive') return emphasis === 'secondary' ? '#50DC2626' : '#FFDC2626';
    return emphasis === 'secondary' ? '#50374151' : '#FF374151';
  }

  private foregroundColor(): string {
    const emphasis = getStringProp(this.params.props, 'emphasis', 'secondary');
    if (emphasis !== 'quiet') return '#FFFFFFFF';
    const intent = getStringProp(this.params.props, 'intent', 'neutral');
    if (intent === 'accent') return '#FF2563EB';
    if (intent === 'destructive') return '#FFDC2626';
    return '#FF111827';
  }
}

@Component
struct IslandSliderNode {
  @Prop params: NativeComponentParams;
  @State private dragging: boolean = false;
  @State private localValue: number = 0;

  aboutToAppear(): void {
    this.localValue = getNumberProp(this.params.props, 'value', 0);
  }

  build() {
    Slider({
      value: this.currentValue(),
      min: getNumberProp(this.params.props, 'min', 0),
      max: Math.max(getNumberProp(this.params.props, 'max', 100), getNumberProp(this.params.props, 'min', 0) + 1),
      step: Math.max(getNumberProp(this.params.props, 'step', 1), 0.01),
      style: SliderStyle.OutSet
    })
      .width('100%')
      .enabled(!getBooleanProp(this.params.props, 'disabled', false))
      .hitTestBehavior(this.hitMode())
      .accessibilityText(this.ariaLabel())
      .accessibilityDescription(this.valueDescription())
      .onChange((value: number, mode: SliderChangeMode) => {
        this.localValue = value;
        if (mode === SliderChangeMode.Begin || mode === SliderChangeMode.Moving) {
          this.dragging = true;
          onNativeComponentEventFromBuilder(this.params.componentId, 'valuechange', { value: value });
        } else if (mode === SliderChangeMode.End || mode === SliderChangeMode.Click) {
          this.dragging = false;
          onNativeComponentEventFromBuilder(this.params.componentId, 'valuecommit', { value: value });
        }
      })
      .onFocus(() => {
        onNativeComponentEventFromBuilder(this.params.componentId, 'focus', { source: 'keyboard' });
      })
      .onBlur(() => {
        onNativeComponentEventFromBuilder(this.params.componentId, 'blur', { source: 'keyboard' });
      })
      .onHover((hovered: boolean) => {
        onNativeComponentEventFromBuilder(
          this.params.componentId,
          hovered ? 'pointerenter' : 'pointerleave',
          { source: 'pointer' }
        );
      })
  }

  private currentValue(): number {
    if (this.dragging) {
      return this.localValue;
    }
    return getNumberProp(this.params.props, 'value', this.localValue);
  }

  private hitMode(): HitTestMode {
    const mode = getStringProp(this.params.props, 'pointerEvents', 'auto');
    return mode === 'none' || mode === 'box-none' ? HitTestMode.None : HitTestMode.Default;
  }

  private ariaLabel(): string {
    return getStringProp(this.params.props, 'aria-label', getStringProp(this.params.props, 'ariaLabel', ''));
  }

  private valueDescription(): string {
    const explicit = getStringProp(
      this.params.props,
      'aria-description',
      getStringProp(this.params.props, 'ariaDescription', '')
    );
    if (explicit.length > 0) return explicit;
    const value = this.currentValue();
    if (getStringProp(this.params.props, 'valueLabel', 'none') === 'time') {
      const total = Math.max(0, Math.round(value));
      const hours = Math.floor(total / 3600);
      const minutes = Math.floor((total % 3600) / 60);
      const seconds = total % 60;
      const mm = minutes.toString().padStart(hours > 0 ? 2 : 1, '0');
      const ss = seconds.toString().padStart(2, '0');
      return hours > 0 ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`;
    }
    return getStringProp(this.params.props, 'valueLabel', 'none') === 'value' ? value.toString() : '';
  }
}

@Builder
function islandViewBuilder(params: NativeComponentParams) {
  IslandViewNode({ params: params })
}

@Builder
function islandTextBuilder(params: NativeComponentParams) {
  IslandTextNode({ params: params })
}

@Builder
function islandTappableBuilder(params: NativeComponentParams) {
  IslandTappableNode({ params: params })
}

@Builder
function islandSliderBuilder(params: NativeComponentParams) {
  IslandSliderNode({ params: params })
}

export function registerIslandBuilders(): void {
  registerNativeComponentBuilder('island/root', wrapBuilder(islandViewBuilder));
  registerNativeComponentBuilder('island/view', wrapBuilder(islandViewBuilder));
  registerNativeComponentBuilder('island/text', wrapBuilder(islandTextBuilder));
  registerNativeComponentBuilder('island/tappable', wrapBuilder(islandTappableBuilder));
  registerNativeComponentBuilder('island/slider', wrapBuilder(islandSliderBuilder));
}
