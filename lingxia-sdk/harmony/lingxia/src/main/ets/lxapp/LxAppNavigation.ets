import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxApp } from './LxApp';
import { LxAppContainer } from './LxAppContainer';
import { onLxappOpened, onLxappClosed } from 'liblingxia.so';

const DOMAIN = 0xA000;
const TAG = 'LingXia.Navigation';

/**
 * LxApp stack item for manual layer management
 */
interface LxAppStackItem {
  appId: string;
  path: string;
  offsetY: number; // For manual animation
  zIndex: number;  // Layer index
}

/**
 * LxApp Navigation with WebView preservation
 * - Hides LxApps instead of destroying them to preserve WebViews
 * - Uses independent hiddenApps Set to avoid miniAppStack mutations
 */
@Component
export struct LxAppNavigation {
  @State isReady: boolean = false
  @State homeAppId: string = ''
  @State homeAppPath: string = ''
  @State errorMessage: string = ''
  @Prop autoOpenHome: boolean = true

  @State private miniAppStack: LxAppStackItem[] = []
  @State private hiddenApps: Set<string> = new Set<string>()

  @Provide navPageInfos: NavPathStack = new NavPathStack()

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, 'LxAppNavigation starting with WebView preservation');
    LxApp.setNavigationInstance(this);
    this.initializeLxApp();
  }

  private async initializeLxApp(): Promise<void> {
    try {
      const instance = LxApp.getInstance();
      const homeAppId = instance.getHomeLxAppId();
      const homeAppPath: string | null = homeAppId ? LxApp.initialRouteCache.get(homeAppId) || null : null;

      if (homeAppId && homeAppPath) {
        this.homeAppId = homeAppId;
        this.homeAppPath = homeAppPath;

        if (this.autoOpenHome) {
          hilog.info(DOMAIN, TAG, `Auto-opening home LxApp: ${this.homeAppId}:${this.homeAppPath}`);
          LxApp.openHomeLxApp();
        } else {
          hilog.info(DOMAIN, TAG, `LxApp initialized, waiting for manual openHomeLxApp call: ${this.homeAppId}:${this.homeAppPath}`);
        }

        this.isReady = true;
        hilog.info(DOMAIN, TAG, `LxApp Navigation ready: ${this.homeAppId}:${this.homeAppPath}`);
      } else {
        this.errorMessage = 'LxApp not initialized. Please call LxApp.initialize() in EntryAbility first.';
        hilog.error(DOMAIN, TAG, 'LxApp not initialized');
      }
    } catch (error) {
      this.errorMessage = `Initialization error: ${error}`;
      hilog.error(DOMAIN, TAG, `Initialization error: ${error}`);
    }
  }

  public openLxApp(appId: string, path: string): void {
    hilog.info(DOMAIN, TAG, `Opening lxapp: ${appId}:${path}`);

    try {
      const result: number = onLxappOpened(appId, path);
      hilog.info(DOMAIN, TAG, `onLxappOpened result: ${result}`);

      const existingIndex = this.miniAppStack.findIndex(item => item.appId === appId);

      if (existingIndex >= 0) {
        hilog.info(DOMAIN, TAG, `LxApp ${appId} already exists, showing and bringing to front`);
        this.showLxApp(appId, existingIndex);
      } else {
        this.createNewLxApp(appId, path);
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open LxApp ${appId}: ${error}`);
    }
  }

  public closeLxApp(appId: string): void {
    hilog.info(DOMAIN, TAG, `Hiding lxapp (preserving WebViews): ${appId}`);

    try {
      const stackIndex = this.miniAppStack.findIndex(item => item.appId === appId);
      if (stackIndex >= 0) {
        this.hideLxAppWithAnimation(appId, stackIndex);
      } else {
        hilog.warn(DOMAIN, TAG, `LxApp not found in stack: ${appId}`);
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to close LxApp ${appId}: ${error}`);
    }
  }

  /**
   * Switch to a specific page within an LxApp
   * @param appId - LxApp ID
   * @param path - Page path to switch to
   * @returns boolean - True if page switch was successful, false otherwise
   */
  public switchPage(appId: string, path: string): boolean {
    // For home app, delegate to the LxAppContainer
    if (appId === this.homeAppId) {
      hilog.info(DOMAIN, TAG, `Switching page in home app ${appId} to ${path}`);
      // The LxAppContainer will handle the page switching via externalPath prop
      return true;
    }

    // For modal apps in stack, find the container and switch page
    const stackItem = this.miniAppStack.find(item => item.appId === appId);
    if (stackItem) {
      hilog.info(DOMAIN, TAG, `Switching page in modal app ${appId} to ${path}`);
      stackItem.path = path;
      // Trigger UI update for the modal container
      this.miniAppStack = [...this.miniAppStack];
      return true;
    }

    hilog.warn(DOMAIN, TAG, `LxApp ${appId} not found for page switching`);
    return false;
  }

  private showLxApp(appId: string, stackIndex: number): void {
    const newHiddenApps = new Set<string>();
    this.hiddenApps.forEach(id => {
      if (id !== appId) {
        newHiddenApps.add(id);
      }
    });
    this.hiddenApps = newHiddenApps;

    this.miniAppStack[stackIndex].offsetY = 0;
    this.miniAppStack[stackIndex].zIndex = 1000 + this.miniAppStack.length;
  }

  private createNewLxApp(appId: string, path: string): void {
    const newStackItem: LxAppStackItem = {
      appId: appId,
      path: path,
      offsetY: 0,
      zIndex: 1000 + this.miniAppStack.length
    };

    this.miniAppStack.push(newStackItem);
    hilog.info(DOMAIN, TAG, `LxApp created: ${appId}:${path}, stack size: ${this.miniAppStack.length}`);
  }

  private hideLxAppWithAnimation(appId: string, stackIndex: number): void {
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
      if (stackIndex < this.miniAppStack.length) {
        this.hiddenApps = new Set(this.hiddenApps).add(appId);
        hilog.info(DOMAIN, TAG, `LxApp ${appId} hidden, WebViews preserved for reuse`);
      }
      onLxappClosed(appId);
    }, 300);
  }

  private isLxAppVisible(appId: string): boolean {
    return !this.hiddenApps.has(appId);
  }

  /**
   * Build modal LxApp container with common styling
   */
  @Builder
  buildModalLxApp(stackItem: LxAppStackItem): void {
    LxAppContainer({
      appId: stackItem.appId,
      initialPath: stackItem.path,
      navigationMode: true,
      closeCallback: 'navigation'
    })
    .width('100%')
    .height('100%')
    .zIndex(this.isLxAppVisible(stackItem.appId) ? stackItem.zIndex : -1)
    .backgroundColor(Color.White)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    .translate({ y: stackItem.offsetY })
    .visibility(this.isLxAppVisible(stackItem.appId) ? Visibility.Visible : Visibility.Hidden)
    .enabled(this.isLxAppVisible(stackItem.appId))
    .onAppear(() => {
      hilog.info(DOMAIN, TAG, `Modal LxApp appeared: ${stackItem.appId}`);
    })
  }

  build() {
    Stack() {
      if (this.isReady && this.homeAppId && this.homeAppPath) {
        LxAppContainer({
          appId: this.homeAppId,
          initialPath: this.homeAppPath,
          navigationMode: false,
          closeCallback: undefined
        })
        .zIndex(0)
      } else if (this.errorMessage) {
        this.buildErrorPage()
      } else {
        this.buildLoadingPage()
      }

      // Only non-home apps go in the stack to avoid duplication
      ForEach(this.miniAppStack, (stackItem: LxAppStackItem, index: number) => {
        if (stackItem.appId !== this.homeAppId) {
          this.buildModalLxApp(stackItem);
        }
      })
    }
    .width('100%')
    .height('100%')
  }

  @Builder
  buildFixedCapsuleButton(appId: string): void {
    if (appId !== LxApp.getInstance().getHomeLxAppId()) {
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
            this.closeLxApp(appId);
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
