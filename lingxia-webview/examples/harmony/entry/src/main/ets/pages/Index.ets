import { MiniApp, MiniAppNavigation } from 'lingxia';

@Entry
@Component
struct Index {
  aboutToAppear() {
    MiniApp.openHomeMiniApp(); 
  }

  build() {
    Column() {
      MiniAppNavigation({ autoOpenHome: false })  // Manual mode
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
  }
}
