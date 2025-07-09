import { LxApp, LxAppNavigation } from 'lingxia';

@Entry
@Component
struct Index {
  aboutToAppear() {
    LxApp.openHomeLxApp();
  }

  build() {
    Column() {
      LxAppNavigation({ autoOpenHome: false })
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
  }
}
