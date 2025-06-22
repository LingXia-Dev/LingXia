import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { MiniApp } from './MiniApp';
import { NavigationBar, type NavigationBarConfig } from './NavigationBar';
import { TabBar, type TabBarConfig, type TabBarItem } from './TabBar';
import { getTabBarConfig, getPageConfig } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Container';

@Component
export struct MiniAppContainer {
  @Prop appId: string
  @Prop initialPath: string
  @Prop navigationMode: boolean = false
  @Prop closeCallback: string = ''
  @State path: string = ''

  @State pageConfig: NavigationBarConfig | null = null
  @State tabBarConfig: TabBarConfig | null = null
  @State selectedTabIndex: number = 0

  // Manual animation control for new miniapps
  @State animationOffsetY: number = 0 // 0 = final position, positive = off-screen bottom

  aboutToAppear() {

    this.path = this.initialPath;
    hilog.info(DOMAIN, TAG, `Container creating for ${this.appId}:${this.path}, isHome=${this.isHomeMiniApp()}`);

    const pageConfigJson: string | null = getPageConfig(this.appId, this.path);
    this.pageConfig = this.parsePageConfig(pageConfigJson);
    //hilog.info(DOMAIN, TAG, `Parsed page config: title=${this.pageConfig?.navigationBarTitleText}, hidden=${this.pageConfig?.hidden}`);

    const webViewController = MiniApp.getInstance().find_webview(this.appId, this.path);
    hilog.info(DOMAIN, TAG, `WebView controller for ${this.appId}:${this.path} = ${webViewController ? 'found' : 'NOT FOUND'}`);

    const tabBarConfigJson: string | null = getTabBarConfig(this.appId);
    this.tabBarConfig = this.parseTabBarConfig(tabBarConfigJson);
    this.setInitialSelectedTab();
    hilog.info(DOMAIN, TAG, `Container initialized: tabs=${this.tabBarConfig?.list.length || 0}, isTabBarApp=${this.tabBarConfig !== null}`);
  }

  build() {
    if (this.isTabBarVertical()) {

      Row() {
        if (this.tabBarConfig && this.tabBarConfig.position === 'left') {
          this.buildTabBar()
        }

        Column() {
          if (this.shouldShowCapsuleButton()) {
            this.buildCapsuleButton()
          }
          if (this.pageConfig && !this.pageConfig.hidden) {
            NavigationBar({
              config: this.pageConfig,
              appId: this.appId,
              currentPath: this.path,
              onBackPressed: (appId: string): boolean => {
                this.handleNavigationBack();
                return true;
              }
            })
          }
          this.buildWebViewArea()
        }
        .layoutWeight(1)
        .height('100%')

        if (this.tabBarConfig && this.tabBarConfig.position === 'right') {
          this.buildTabBar()
        }
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .expandSafeArea(this.shouldExpandSafeAreaForContainer() ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
      .onAppear(() => {
        MiniApp.getInstance().on_page_show(this.appId, this.path);
      })
    } else if (this.isTabBarTransparent()) {

      Stack({ alignContent: this.getTabBarAlignment() }) {
        Column() {
          this.buildNavigationBar()
          this.buildWebViewArea()
        }
        .width('100%')
        .height('100%')

        this.buildTabBar()
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .expandSafeArea(this.shouldExpandSafeAreaForContainer() ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
      .onAppear(() => {
        MiniApp.getInstance().on_page_show(this.appId, this.path);
      })
    } else {
      Stack() {
        Column() {
          if (this.tabBarConfig && this.tabBarConfig.position === 'top') {
            this.buildTabBar()
          }

          this.buildNavigationBar()
          this.buildWebViewArea()

          if (this.tabBarConfig && (this.tabBarConfig.position === 'bottom' || !this.tabBarConfig.position)) {
            this.buildTabBar()
          }
        }
        .width('100%')
        .height('100%')

        this.buildStandaloneCapsuleButton()
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .expandSafeArea(this.shouldExpandSafeAreaForContainer() ? [SafeAreaType.SYSTEM] : [], this.shouldExpandSafeAreaForContainer() ? [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM] : [])
      .onAppear(() => {
        MiniApp.getInstance().on_page_show(this.appId, this.path);
      })
    }
  }

  private isHomeMiniApp(): boolean {
    const miniApp = MiniApp.getInstance();
    const homeAppId = miniApp.getHomeMiniAppId();
    return this.appId === homeAppId;
  }

  private shouldExpandSafeAreaForContainer(): boolean {
    const shouldExpand = true;
    hilog.info(DOMAIN, TAG, `Always expand safe area for full screen coverage: ${this.appId}`);
    return shouldExpand;
  }

  /**
   * Check if TabBar is transparent
   * When transparent, TabBar overlays WebView; when opaque, they are separate
   */
  private isTabBarTransparent(): boolean {
    return this.tabBarConfig?.backgroundColor === "transparent";
  }

  /**
   * Check if TabBar is vertical (left/right position)
   */
  private isTabBarVertical(): boolean {
    return this.tabBarConfig?.position === 'left' || this.tabBarConfig?.position === 'right';
  }

  /**
   * Get TabBar alignment for Stack layout
   */
  private getTabBarAlignment(): Alignment {
    const position = this.tabBarConfig?.position || 'bottom';
    switch (position) {
      case 'top':
        return Alignment.Top;
      case 'left':
        return Alignment.Start;
      case 'right':
        return Alignment.End;
      case 'bottom':
      default:
        return Alignment.Bottom;
    }
  }

  /**
   * Build TabBar component
   */
  @Builder
  buildTabBar(): void {
    if (this.tabBarConfig && this.tabBarConfig.list.length > 0) {
      TabBar({
        config: this.tabBarConfig,
        appId: this.appId,
        selectedIndex: this.selectedTabIndex,
        onTabSelected: (appId: string, index: number, item: TabBarItem): void => this.handleTabSelected(index)
      })
    }
  }

  /**
   * Build NavigationBar component (shared by both branches)
   */
  @Builder
  buildNavigationBar(): void {
    if (this.pageConfig && !this.pageConfig.hidden) {
      NavigationBar({
        config: this.pageConfig,
        appId: this.appId,
        currentPath: this.path,
        showCapsuleButton: false,
        onBackPressed: (appId: string): boolean => {
          this.handleNavigationBack();
          return true;
        },
        onCapsuleMore: () => this.handleCapsuleMore(),
        onCapsuleClose: () => this.handleCapsuleClose()
      })
    }
  }

  @Builder
  buildStandaloneCapsuleButton(): void {
    if (this.shouldShowCapsuleButton()) {
      Stack() {
        Row() {
          Button() {
            this.buildThreeDots()
          }
          .width(40)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
            this.handleCapsuleMore();
          })

          Column()
            .width(0.5)
            .height(18)
            .backgroundColor('#DDDDDD')

          Button() {
            this.buildCloseIcon()
          }
          .width(40)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, `Capsule close button pressed for appId: ${this.appId}`);
            this.handleCapsuleClose();
          })
        }
        .backgroundColor('rgba(255,255,255,0.9)')
        .borderRadius(20)
        .border({ width: 0.5, color: '#DDDDDD' })
      }
      .position({ x: '100%', y: 44 })
      .translate({ x: -95, y: 6 })
      .width(95)
      .height(36)
      .zIndex(9999)
    }
  }

  /**
   * Check if should show capsule button
   * homeminiapp doesn't show capsule button, other miniapps do
   */
  private shouldShowCapsuleButton(): boolean {
    const isHome = this.isHomeMiniApp();
    const shouldShow = !isHome;
    hilog.info(DOMAIN, TAG, `shouldShowCapsuleButton: appId=${this.appId}, isHome=${isHome}, shouldShow=${shouldShow}`);
    return shouldShow;
  }

  /**
   * Build capsule button
   */
  @Builder
  buildCapsuleButton(): void {
    Row() {
      Blank()

      Row() {
        Button() {
          this.buildThreeDots()
        }
        .width(44)
        .height(36)
        .backgroundColor(Color.Transparent)
        .onClick(() => this.handleCapsuleMore())

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
        .enabled(true)
        .onClick(() => {
          this.handleCapsuleClose();
        })
        .onTouch((event) => {
          hilog.info(DOMAIN, TAG, `Close button touched: ${event.type}`);
        })
      }
      .backgroundColor('rgba(255,255,255,0.9)')
      .borderRadius(20)
      .border({ width: 0.5, color: '#DDDDDD' })
      .margin({ top: 8, right: 12 })
    }
    .width('100%')
    .height(64)
    .backgroundColor(Color.Transparent)
    .zIndex(9999)
  }

  /**
   * Build three dots icon
   */
  @Builder
  buildThreeDots(): void {
    Row() {
      // Three custom drawn dots - center dot larger, side dots smaller (matching Android)
      Circle()
        .width(3)
        .height(3)
        .fill('#000000')

      Circle()
        .width(5)
        .height(5)
        .fill('#000000')
        .margin({ left: 4 })

      Circle()
        .width(3)
        .height(3)
        .fill('#000000')
        .margin({ left: 4 })
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  /**
   * Build close icon (outer circle LINE with inner dot, transparent middle)
   */
  @Builder
  buildCloseIcon(): void {
    Stack() {
      // Outer circle - use stroke instead of border to ensure circular shape
      Circle()
        .width(16)
        .height(16)
        .fillOpacity(0) // Transparent fill
        .stroke('#000000')
        .strokeWidth(2.5) // Thicker stroke matching Android

      // Inner dot (thick dot in center) - radius should be outer radius / 2.5
      Circle()
        .width(6.5)
        .height(6.5)
        .fill('#000000')
    }
  }

  /**
   * Build WebView area
   */
  @Builder
  buildWebViewArea(): void {
    if (this.navigationMode) {
      Stack() {
        if (this.tabBarConfig && this.tabBarConfig.list.length > 0) {
          ForEach(this.tabBarConfig.list, (item: TabBarItem, index: number) => {
            Web({
              src: '',
              controller: MiniApp.getInstance().find_webview(this.appId, item.pagePath)
            })
              .width('100%')
              .height('100%')
              .visibility(this.selectedTabIndex === index ? Visibility.Visible : Visibility.Hidden)
              .onAppear(() => {
                if (this.selectedTabIndex === index) {
                  hilog.info(DOMAIN, TAG, `WebView[${index}] appeared, triggering onPageShow for path: ${item.pagePath}`);
                  MiniApp.getInstance().on_page_show(this.appId, item.pagePath);
                }
              })
          })
        } else {
          Web({
            src: '',
            controller: MiniApp.getInstance().find_webview(this.appId, this.path)
          })
            .width('100%')
            .height('100%')
            .onAppear(() => {
              hilog.info(DOMAIN, TAG, `WebView appeared, triggering onPageShow for path: ${this.path}`);
              MiniApp.getInstance().on_page_show(this.appId, this.path);
            })
        }
      }
      .width('100%')
      .height('100%')
      .layoutWeight(this.isTabBarTransparent() ? 0 : 1)
      .backgroundColor(Color.White)
      .clip(true)
    } else {
      Stack() {
        if (this.tabBarConfig && this.tabBarConfig.list.length > 0) {
          ForEach(this.tabBarConfig.list, (item: TabBarItem, index: number) => {
            Web({
              src: '',
              controller: MiniApp.getInstance().find_webview(this.appId, item.pagePath)
            })
              .width('100%')
              .height('100%')
              .visibility(this.selectedTabIndex === index ? Visibility.Visible : Visibility.Hidden)
              .onAppear(() => {
                if (this.selectedTabIndex === index) {
                  hilog.info(DOMAIN, TAG, `WebView[${index}] appeared, triggering onPageShow for path: ${item.pagePath}`);
                  MiniApp.getInstance().on_page_show(this.appId, item.pagePath);
                }
              })
          })
        } else {
          Web({
            src: '',
            controller: MiniApp.getInstance().find_webview(this.appId, this.path)
          })
            .width('100%')
            .height('100%')
            .onAppear(() => {
              hilog.info(DOMAIN, TAG, `WebView appeared, triggering onPageShow for path: ${this.path}`);
              MiniApp.getInstance().on_page_show(this.appId, this.path);
            })
        }
      }
      .width('100%')
      .height('100%')
      .layoutWeight(this.isTabBarTransparent() ? 0 : 1)
    }
  }

  private setInitialSelectedTab() {
    if (this.tabBarConfig && this.tabBarConfig.list.length > 0) {
      // Find corresponding tab based on current path
      const currentTabIndex = this.tabBarConfig.list.findIndex((item: TabBarItem) =>
        this.path.includes(item.pagePath)
      );
      this.selectedTabIndex = currentTabIndex >= 0 ? currentTabIndex : 0;

      hilog.info(DOMAIN, TAG, `setInitialSelectedTab: appId=${this.appId}, path=${this.path}, selectedTabIndex=${this.selectedTabIndex}`);
      this.tabBarConfig.list.forEach((item, index) => {
        hilog.info(DOMAIN, TAG, `Tab ${index}: path=${item.pagePath}, selected=${index === this.selectedTabIndex} (appId: ${this.appId})`);
      });
    }
  }

  private handleTabSelected(index: number) {
    hilog.info(DOMAIN, TAG, `Tab selected: ${index}, current: ${this.selectedTabIndex}`);
    if (this.tabBarConfig && index < this.tabBarConfig.list.length) {
      this.switchToTabByIndex(index);
    }
  }

  /**
   * Switch to tab by index
   */
  private switchToTabByIndex(index: number): void {
    if (!this.tabBarConfig || index < 0 || index >= this.tabBarConfig.list.length) {
      hilog.warn(DOMAIN, TAG, `Invalid tab index: ${index}`);
      return;
    }

    const newPath = this.tabBarConfig.list[index].pagePath;

    // Skip if already on this tab
    if (this.selectedTabIndex === index) {
      hilog.info(DOMAIN, TAG, `Already on tab ${index}, skipping switch`);
      return;
    }

    hilog.info(DOMAIN, TAG, `Switching from tab ${this.selectedTabIndex} to tab ${index}, path: ${newPath}`);

    // Update selected index to trigger UI visibility change
    this.selectedTabIndex = index;

    // Update current path to the new tab path
    this.path = newPath;

    // If this is home miniapp, update the manager's current path state
    if (this.isHomeMiniApp()) {
      const manager = MiniApp.managerInstance;
      if (manager) {
        manager.updateHomeMiniAppCurrentPath(newPath);
      }
    }

    const pageConfigJson: string | null = getPageConfig(this.appId, newPath);
    const newPageConfig = this.parsePageConfig(pageConfigJson);
    this.pageConfig = newPageConfig;

    hilog.info(DOMAIN, TAG, `Page config updated: hidden=${newPageConfig.hidden}, title=${newPageConfig.navigationBarTitleText}`);

    // Directly trigger onPageShow for the new tab
    hilog.info(DOMAIN, TAG, `Tab switched to ${index}, triggering onPageShow for path: ${newPath}`);

    // Use setTimeout to ensure visibility change is processed first
    setTimeout(() => {
      MiniApp.getInstance().on_page_show(this.appId, newPath);
    }, 50);
  }

  /**
   * Switch to page by path
   * Called by MiniApp.switchToPage API
   */
  public switchToPageByPath(path: string): boolean {
    hilog.info(DOMAIN, TAG, `switchToPageByPath called: ${path}`);

    // Use cached TabBar config to find tab index
    if (this.tabBarConfig) {
      const tabIndex = this.tabBarConfig.list.findIndex(item =>
        item.pagePath === path || item.pagePath.includes(path)
      );

      if (tabIndex >= 0) {
        hilog.info(DOMAIN, TAG, `Found tab ${tabIndex} for path ${path}, switching tab`);
        this.switchToTabByIndex(tabIndex);
        return true;
      }
    }

    hilog.warn(DOMAIN, TAG, `Path ${path} not found in cached TabBar config`);
    return false;
  }

  private handleNavigationBack() {
    hilog.info(DOMAIN, TAG, 'Navigation back pressed');
    // TODO: Implement navigation back logic
  }

  private handleCapsuleMore(): void {
    hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
    // TODO: Implement more menu
  }

  private handleCapsuleClose(): void {
    // Close second miniapp if this is not home miniapp
    if (!this.isHomeMiniApp()) {
        MiniApp.closeMiniApp(this.appId);
    }
  }

  /**
   * Parse page config JSON
   */
  private parsePageConfig(json: string | null): NavigationBarConfig {
    if (!json) {
      return { hidden: true };
    }

    try {
      const config = JSON.parse(json) as Record<string, Object>;
      return {
        navigationBarTitleText: (config.navigationBarTitleText as string) || '',
        navigationBarBackgroundColor: (config.navigationBarBackgroundColor as string) || '#ffffff',
        navigationBarTextStyle: (config.navigationBarTextStyle as string) || 'black',
        navigationStyle: (config.navigationStyle as string) || 'default',
        backgroundColor: (config.backgroundColor as string) || '#ffffff',
        hidden: (config.hidden as boolean) || false
      };
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to parse page config JSON: ${error}`);
      return { hidden: true };
    }
  }

  /**
   * Parse TabBar config JSON
   */
  private parseTabBarConfig(json: string | null): TabBarConfig | null {
    if (!json) {
      return null;
    }

    try {
      const config = JSON.parse(json) as Record<string, Object>;
      const tabConfig: TabBarConfig = {
        color: (config.color as string) || '#666666',
        selectedColor: (config.selectedColor as string) || '#1677FF',
        backgroundColor: (config.backgroundColor as string) || '#ffffff',
        borderStyle: (config.borderStyle as string) || 'black',
        position: (config.position as string) || 'bottom',
        list: (config.list as TabBarItem[]) || []
      };

      hilog.info(DOMAIN, TAG, `Parsed TabBar config: ${tabConfig.list.length} items`);
      tabConfig.list.forEach((item, index) => {
        hilog.info(DOMAIN, TAG, `Tab ${index}: path=${item.pagePath}, text=${item.text || 'none'}, icon=${item.iconPath || 'none'}`);
      });

      return tabConfig;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to parse TabBar config JSON: ${error}`);
      return null;
    }
  }
}
