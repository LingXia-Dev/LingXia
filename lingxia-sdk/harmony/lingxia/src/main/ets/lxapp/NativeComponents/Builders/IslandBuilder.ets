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
    .hitTestBehavior(this.hitMode())
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
}

@Component
struct IslandTextNode {
  @Prop params: NativeComponentParams;

  build() {
    Text(getStringProp(this.params.props, 'text', ''))
      .fontColor('#FFFFFF')
      .fontSize(12)
      .width('100%')
      .height('100%')
      .hitTestBehavior(HitTestMode.None)
  }
}

@Component
struct IslandTappableNode {
  @Prop params: NativeComponentParams;

  build() {
    Button(this.label())
      .width('100%')
      .height('100%')
      .backgroundColor('#CC2A2A2A')
      .fontColor('#FFFFFF')
      .enabled(!getBooleanProp(this.params.props, 'disabled', false))
      .onClick(() => {
        onNativeComponentEventFromBuilder(this.params.componentId, 'press', { source: 'pointer' });
      })
  }

  private label(): string {
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
  }

  private currentValue(): number {
    if (this.dragging) {
      return this.localValue;
    }
    return getNumberProp(this.params.props, 'value', this.localValue);
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
