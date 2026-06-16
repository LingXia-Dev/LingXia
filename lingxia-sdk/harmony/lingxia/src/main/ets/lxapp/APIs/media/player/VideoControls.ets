/**
 * Shared Video Controls Component
 * Provides consistent video player controls across inline, fullscreen, and preview modes
 */
import { formatVideoTime } from './VideoUtils';

// Control size presets
export class VideoControlSize {
  static readonly COMPACT: string = 'compact';
  static readonly STANDARD: string = 'standard';
}

// Dimension class for type safety
class ControlDimensions {
  progressHeight: number = 32;
  controlHeight: number = 36;
  iconSize: number = 22;
  buttonSize: number = 40;
  volumeWidth: number = 60;
  trackThickness: number = 3;
  blockSize: number = 14;
  fontSize: number = 11;
  paddingLeft: number = 8;
  paddingRight: number = 8;
  paddingBottom: number = 8;
  paddingTop: number = 6;
  marginTop: number = 2;
}

function getCompactDimensions(): ControlDimensions {
  const dim = new ControlDimensions();
  return dim;
}

function getStandardDimensions(): ControlDimensions {
  const dim = new ControlDimensions();
  dim.progressHeight = 40;
  dim.controlHeight = 48;
  dim.iconSize = 28;
  dim.buttonSize = 48;
  dim.volumeWidth = 100;
  dim.trackThickness = 4;
  dim.blockSize = 16;
  dim.fontSize = 12;
  dim.paddingLeft = 12;
  dim.paddingRight = 12;
  dim.paddingBottom = 20;
  dim.paddingTop = 8;
  dim.marginTop = 4;
  return dim;
}

// Reusable video controls component
@Component
export struct VideoControlsView {
  @Prop isPlaying: boolean = false;
  @Prop currentTime: number = 0;
  @Prop duration: number = 0;
  @Prop sliderValue: number = 0;
  @Prop volume: number = 0.8;
  @Prop muted: boolean = false;
  @Prop controlSize: string = VideoControlSize.COMPACT;  // renamed from 'size' to avoid ArkTS reserved property
  @Prop showProgressBar: boolean = true;
  @Prop showVolumeSlider: boolean = true;
  @Prop showFullscreenButton: boolean = false;
  @Prop showExitButton: boolean = false;
  @Prop showTimeRemaining: boolean = false;
  @Prop qualities: string[] = [];           // Available quality options
  @Prop currentQuality: string = '';        // Currently selected quality
  @Prop playbackRates: number[] = [];       // Available playback rates
  @Prop currentPlaybackRate: number = 1.0;  // Current playback rate
  @State showSettingsMenu: boolean = false;
  @State settingsMenuType: string = '';     // 'main', 'quality', 'speed'

  onSliderChange: (value: number, mode: SliderChangeMode) => void = () => {};
  onPlayPauseClick: () => void = () => {};
  onMuteClick: () => void = () => {};
  onVolumeChange: (value: number) => void = () => {};
  onFullscreenClick: () => void = () => {};
  onExitClick: () => void = () => {};
  onQualitySelect: (quality: string) => void = () => {};
  onPlaybackRateSelect: (rate: number) => void = () => {};

  private getDim(): ControlDimensions {
    return this.controlSize === VideoControlSize.STANDARD ? getStandardDimensions() : getCompactDimensions();
  }

  private hasSettings(): boolean {
    return this.qualities.length > 0 || this.playbackRates.length > 0;
  }

  private closeSettingsMenu(): void {
    this.showSettingsMenu = false;
    this.settingsMenuType = '';
  }

  private formatRate(rate: number): string {
    return rate === Math.floor(rate) ? `${Math.floor(rate)}x` : `${rate}x`;
  }

  @Builder
  buildSettingsOverlay() {
    // Use Column as container - menu positioned at bottom-right via alignment
    Column() {
      // Spacer to push menu to bottom
      Blank()
        .layoutWeight(1)
        .onClick(() => this.closeSettingsMenu())

      // Bottom row: spacer + menu
      Row() {
        // Left spacer (click to dismiss)
        Blank()
          .layoutWeight(1)
          .onClick(() => this.closeSettingsMenu())

        // Menu Content - use Scroll for many items
        Scroll() {
          Column() {
            if (this.settingsMenuType === 'main') {
              // Main menu with Quality and Speed options
              if (this.qualities.length > 0) {
                Row() {
                  Column() {
                    Text($r('app.string.lx_video_quality'))
                      .fontSize(14)
                      .fontColor(Color.White)
                      .textAlign(TextAlign.Start)
                      .width('100%')
                    Text(this.currentQuality || this.qualities[0] || 'Auto')
                      .fontSize(12)
                      .fontColor('rgba(255,255,255,0.6)')
                      .textAlign(TextAlign.Start)
                      .width('100%')
                      .margin({ top: 2 })
                  }
                  .layoutWeight(1)
                  .alignItems(HorizontalAlign.Start)

                  Text('›')
                    .fontSize(16)
                    .fontColor('rgba(255,255,255,0.4)')
                    .margin({ left: 4 })
                }
                .width('100%')
                .height(44)
                .padding({ left: 12, right: 12 })
                .onClick(() => { this.settingsMenuType = 'quality'; })
              }
              if (this.playbackRates.length > 0) {
                Row() {
                  Column() {
                    Text($r('app.string.lx_video_speed'))
                      .fontSize(14)
                      .fontColor(Color.White)
                      .textAlign(TextAlign.Start)
                      .width('100%')
                    Text(this.formatRate(this.currentPlaybackRate))
                      .fontSize(12)
                      .fontColor('rgba(255,255,255,0.6)')
                      .textAlign(TextAlign.Start)
                      .width('100%')
                      .margin({ top: 2 })
                  }
                  .layoutWeight(1)
                  .alignItems(HorizontalAlign.Start)

                  Text('›')
                    .fontSize(16)
                    .fontColor('rgba(255,255,255,0.4)')
                    .margin({ left: 4 })
                }
                .width('100%')
                .height(44)
                .padding({ left: 12, right: 12 })
                .onClick(() => { this.settingsMenuType = 'speed'; })
              }
            } else if (this.settingsMenuType === 'quality') {
              // Quality submenu
              ForEach(this.qualities, (quality: string) => {
                Row() {
                  Text(quality)
                    .fontSize(14)
                    .fontColor(Color.White)
                    .layoutWeight(1)
                  if (quality === (this.currentQuality || this.qualities[0])) {
                    Text('✓')
                      .fontSize(14)
                      .fontColor('#007AFF')
                  }
                }
                .width('100%')
                .height(36)
                .padding({ left: 12, right: 12 })
                .backgroundColor(quality === (this.currentQuality || this.qualities[0]) ? 'rgba(0,122,255,0.2)' : Color.Transparent)
                .borderRadius(8)
                .onClick(() => {
                  this.onQualitySelect(quality);
                  this.closeSettingsMenu();
                })
              })
            } else if (this.settingsMenuType === 'speed') {
              // Speed submenu
              ForEach(this.playbackRates, (rate: number) => {
                Row() {
                  Text(this.formatRate(rate))
                    .fontSize(14)
                    .fontColor(Color.White)
                    .layoutWeight(1)
                  if (rate === this.currentPlaybackRate) {
                    Text('✓')
                      .fontSize(14)
                      .fontColor('#007AFF')
                  }
                }
                .width('100%')
                .height(36)
                .padding({ left: 12, right: 12 })
                .backgroundColor(rate === this.currentPlaybackRate ? 'rgba(0,122,255,0.2)' : Color.Transparent)
                .borderRadius(8)
                .onClick(() => {
                  this.onPlaybackRateSelect(rate);
                  this.closeSettingsMenu();
                })
              })
            }
          }
          .width('100%')
        }
        .width(180)
        .constraintSize({ maxHeight: 200 })
        .padding(8)
        .backgroundColor('rgba(46,46,46,0.97)')
        .borderRadius(12)
        .scrollBar(BarState.Off)
        .margin({ right: this.getDim().paddingRight })
      }
      .width('100%')
      .alignItems(VerticalAlign.Bottom)
      .margin({ bottom: this.getDim().controlHeight + this.getDim().paddingBottom + 12 })
    }
    .width('100%')
    .height('100%')
  }

  build() {
    Stack({ alignContent: Alignment.BottomEnd }) {
      Column() {
        // Progress bar row
        if (this.showProgressBar) {
          Row() {
            Slider({ value: this.sliderValue, min: 0, max: 100, step: 0.1 })
              .layoutWeight(1)
              .blockColor(Color.White)
              .trackColor('rgba(255,255,255,0.3)')
              .selectedColor(Color.White)
              .trackThickness(this.getDim().trackThickness)
              .blockSize({ width: this.getDim().blockSize, height: this.getDim().blockSize })
              .sliderInteractionMode(SliderInteraction.SLIDE_AND_CLICK)
              .onChange((value: number, mode: SliderChangeMode) => this.onSliderChange(value, mode))

            Text(this.showTimeRemaining
              ? `-${formatVideoTime(Math.max(0, this.duration - this.currentTime))}`
              : formatVideoTime(this.duration))
              .fontSize(this.getDim().fontSize)
              .fontColor(Color.White)
              .margin({ left: 6 })
          }
          .width('100%')
          .height(this.getDim().progressHeight)
          .alignItems(VerticalAlign.Center)
        }

        // Control buttons row
        Row() {
          // Play/Pause button
          Row() {
            Image(this.isPlaying ? $rawfile('icons/icon_pause.svg') : $rawfile('icons/icon_play.svg'))
              .width(this.getDim().iconSize)
              .height(this.getDim().iconSize)
              .fillColor(Color.White)
          }
          .width(this.getDim().buttonSize)
          .height(this.getDim().controlHeight)
          .justifyContent(FlexAlign.Center)
          .onClick(() => this.onPlayPauseClick())

          // Volume icon
          Row() {
            Image(this.muted ? $rawfile('icons/icon_volume_off.svg') : $rawfile('icons/icon_volume_on.svg'))
              .width(this.getDim().iconSize - 4)
              .height(this.getDim().iconSize - 4)
              .fillColor(Color.White)
          }
          .width(this.getDim().buttonSize - 4)
          .height(this.getDim().controlHeight)
          .justifyContent(FlexAlign.Center)
          .onClick(() => this.onMuteClick())

          // Volume slider
          if (this.showVolumeSlider) {
            Slider({ value: this.muted ? 0 : this.volume * 100, min: 0, max: 100, step: 1 })
              .width(this.getDim().volumeWidth)
              .blockColor(Color.White)
              .trackColor('rgba(255,255,255,0.3)')
              .selectedColor(Color.White)
              .trackThickness(this.getDim().trackThickness - 1)
              .blockSize({ width: this.getDim().blockSize - 2, height: this.getDim().blockSize - 2 })
              .onChange((value: number) => this.onVolumeChange(value))
          }

          Blank().layoutWeight(1)

          // Settings button (if has qualities or playback rates)
          if (this.hasSettings()) {
            Row() {
              Image($rawfile('icons/icon_settings.svg'))
                .width(this.getDim().iconSize - 2)
                .height(this.getDim().iconSize - 2)
            }
            .width(this.getDim().buttonSize - 4)
            .height(this.getDim().controlHeight)
            .justifyContent(FlexAlign.Center)
            .onClick(() => {
              this.showSettingsMenu = !this.showSettingsMenu;
              this.settingsMenuType = this.showSettingsMenu ? 'main' : '';
            })
          }

          // Exit button (fullscreen mode)
          if (this.showExitButton) {
            Row() {
              Image($rawfile('icons/icon_fullscreen_exit.svg'))
                .width(this.getDim().iconSize)
                .height(this.getDim().iconSize)
                .fillColor(Color.White)
            }
            .width(this.getDim().buttonSize)
            .height(this.getDim().controlHeight)
            .justifyContent(FlexAlign.Center)
            .onClick(() => this.onExitClick())
          }

          // Fullscreen button (inline mode)
          if (this.showFullscreenButton && !this.showExitButton) {
            Row() {
              Image($rawfile('icons/icon_fullscreen_enter.svg'))
                .width(this.getDim().iconSize)
                .height(this.getDim().iconSize)
                .fillColor(Color.White)
            }
            .width(this.getDim().buttonSize)
            .height(this.getDim().controlHeight)
            .justifyContent(FlexAlign.Center)
            .onClick(() => this.onFullscreenClick())
          }
        }
        .width('100%')
        .height(this.getDim().controlHeight)
        .alignItems(VerticalAlign.Center)
        .margin({ top: this.getDim().marginTop })
      }
      .width('100%')
      .padding({
        left: this.getDim().paddingLeft,
        right: this.getDim().paddingRight,
        bottom: this.getDim().paddingBottom,
        top: this.getDim().paddingTop
      })
      .linearGradient({
        direction: GradientDirection.Bottom,
        colors: [['rgba(0,0,0,0)', 0], ['rgba(0,0,0,0.7)', 1]]
      })

      if (this.showSettingsMenu) {
        this.buildSettingsOverlay()
      }
    }
    .width('100%')
    .height('100%')
  }
}

// Center play button component
@Component
export struct CenterPlayButton {
  @Prop buttonSize: number = 72;  // renamed from 'size' to avoid ArkTS reserved property
  onButtonClick: () => void = () => {};  // renamed from 'onClick' to avoid ArkTS reserved property

  build() {
    Row() {
      Image($rawfile('icons/icon_play.svg'))
        .width(this.buttonSize / 2)
        .height(this.buttonSize / 2)
        .fillColor(Color.White)
    }
    .width(this.buttonSize)
    .height(this.buttonSize)
    .justifyContent(FlexAlign.Center)
    .borderRadius(this.buttonSize / 2)
    .backgroundColor('rgba(0,0,0,0.5)')
    .zIndex(2)
    .hitTestBehavior(HitTestMode.Block)
    .onClick(() => this.onButtonClick())
  }
}
