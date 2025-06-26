
import { hilog } from '@kit.PerformanceAnalysisKit';
import { MiniApp } from './MiniApp';
import { MiniAppContainer } from './MiniAppContainer';
import { onMiniappOpened, onMiniappClosed } from 'liblingxia.so';

const DOMAIN = 0xA000;
const TAG = 'LingXia.Navigation';

/**
 * MiniApp stack item for manual layer management
 */
interface MiniAppStackItem {
  appId: string;
  path: string;
  offsetY: number; // For manual animation
  zIndex: number;  // Layer index
}

@Component
export struct MiniAppNavigation {
  @State isReady: boolean = false
  @State homeAppId: string = ''
  @State homeAppPath: string = ''
  @State errorMessage: string = ''

  @State private miniAppStack: MiniAppStackItem[] = []

  // Optional parameter: whether to auto open home MiniApp (default: true)
  @Prop autoOpenHome: boolean = true

  @Provide navPageInfos: NavPathStack = new NavPathStack()

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, 'MiniAppNavigation starting - Manual Stack-based architecture');

    MiniApp.setNavigationInstance(this);

    this.initializeMiniApp();
  }

  private async initializeMiniApp(): Promise<void> {
    try {
      // MiniApp should already be initialized in EntryAbility
      // Just get the home app info from the already initialized instance
      const instance = MiniApp.getInstance();
      const homeAppId = instance.getHomeMiniAppId();
      const homeAppPath = instance.getHomeMiniAppInitialRoute();

      if (homeAppId && homeAppPath) {
        this.homeAppId = homeAppId;
        this.homeAppPath = homeAppPath;

        if (this.autoOpenHome) {
          // Auto mode: internally call openHomeMiniApp
          hilog.info(DOMAIN, TAG, `Auto-opening home MiniApp: ${this.homeAppId}:${this.homeAppPath}`);
          MiniApp.openHomeMiniApp();
          hilog.info(DOMAIN, TAG, `Auto openHomeMiniApp completed`);
        } else {
          // Manual mode: wait for external openHomeMiniApp call
          hilog.info(DOMAIN, TAG, `MiniApp initialized, waiting for manual openHomeMiniApp call: ${this.homeAppId}:${this.homeAppPath}`);
        }

        this.isReady = true;
        hilog.info(DOMAIN, TAG, `MiniApp Navigation ready: ${this.homeAppId}:${this.homeAppPath}`);
      } else {
        this.errorMessage = 'MiniApp not initialized. Please call MiniApp.initialize() in EntryAbility first.';
        hilog.error(DOMAIN, TAG, 'MiniApp not initialized. Please call MiniApp.initialize() in EntryAbility first.');
      }
    } catch (error) {
      this.errorMessage = `Initialization error: ${error}`;
      hilog.error(DOMAIN, TAG, `Failed to initialize MiniApp: ${error}`);
    }
  }

  public openMiniApp(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `Opening miniapp with manual Stack animation: ${appId}:${path}`);

    try {
      const result: number = onMiniappOpened(appId, path);
      hilog.info(DOMAIN, TAG, `onMiniappOpened result: ${result}`);

      const newStackItem: MiniAppStackItem = {
        appId: appId,
        path: path,
        offsetY: 0,
        zIndex: 1000 + this.miniAppStack.length
      };

      this.miniAppStack.push(newStackItem);
      hilog.info(DOMAIN, TAG, `MiniApp opened with manual Stack animation: ${appId}:${path}, stack size: ${this.miniAppStack.length}`);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open MiniApp ${appId}: ${error}`);
    }
  }

  public closeMiniApp(appId: string): void {
    hilog.info(DOMAIN, TAG, `Closing miniapp with manual Stack animation: ${appId}`);

    try {
      const stackIndex = this.miniAppStack.findIndex(item => item.appId === appId);

      if (stackIndex >= 0) {

        animateTo({
          duration: 250,
          curve: Curve.EaseIn,
          playMode: PlayMode.Normal
        }, () => {
          if (stackIndex < this.miniAppStack.length) {
            this.miniAppStack[stackIndex].offsetY = 800;
          }
        });

        setTimeout(() => {
          this.miniAppStack = this.miniAppStack.filter(item => item.appId !== appId);
          onMiniappClosed(appId);
        }, 300);

      } else {
        hilog.warn(DOMAIN, TAG, `MiniApp not found in stack: ${appId}`);
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to close MiniApp ${appId}: ${error}`);
    }
  }

  build() {
    Stack() {
      if (this.isReady && this.homeAppId && this.homeAppPath) {
        MiniAppContainer({
          appId: this.homeAppId,
          initialPath: this.homeAppPath,
          navigationMode: false,
          closeCallback: 'navigation'
        })
        .zIndex(0)
      } else if (this.errorMessage) {
        this.buildErrorPage()
      } else {
        this.buildLoadingPage()
      }

      ForEach(this.miniAppStack, (stackItem: MiniAppStackItem, index: number) => {
        MiniAppContainer({
          appId: stackItem.appId,
          initialPath: stackItem.path,
          navigationMode: true,
          closeCallback: 'navigation'
        })
        .width('100%')
        .height('100%')
        .zIndex(stackItem.zIndex)
        .backgroundColor(Color.White)
        .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
      })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  buildFixedCapsuleButton(appId: string): void {
    if (appId !== MiniApp.getInstance().getHomeMiniAppId()) {
      Stack() {
        Row() {
          Button() {
            this.buildThreeDots()
          }
          .width(44)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
          })

          Column()
            .width(0.5)
            .height(18)
            .backgroundColor('#DDDDDD')

          Button() {
            this.buildCloseIcon()
          }
          .width(44)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, `Capsule close button pressed for appId: ${appId}`);
            this.closeMiniApp(appId);
          })
        }
        .backgroundColor('rgba(255,255,255,0.9)')
        .borderRadius(20)
        .border({ width: 0.5, color: '#DDDDDD' })
        .margin({ top: 52, right: 12 })
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .zIndex(9999)
      .alignContent(Alignment.TopEnd)
    }
  }

  /**
   * Build three dots icon
   */
  @Builder
  buildThreeDots(): void {
    Row() {
      Circle().width(3).height(3).fill('#000000')
      Circle().width(5).height(5).fill('#000000').margin({ left: 4 })
      Circle().width(3).height(3).fill('#000000').margin({ left: 4 })
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  /**
   * Build close icon
   */
  @Builder
  buildCloseIcon(): void {
    Stack() {
      Circle()
        .width(16)
        .height(16)
        .fillOpacity(0)
        .stroke('#000000')
        .strokeWidth(2.5)
      Circle()
        .width(6.5)
        .height(6.5)
        .fill('#000000')
    }
  }

  @Builder
  buildErrorPage() {
    Column() {
      Text('⚠️')
        .fontSize(32)
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .width('100%')
    .height('100%')
  }

  /**
   * Build clean loading page
   */
  @Builder
  buildLoadingPage() {
    Column() {
      LoadingProgress()
        .width(24)
        .height(24)
        .color('#1677FF')
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .width('100%')
    .height('100%')
  }
}
