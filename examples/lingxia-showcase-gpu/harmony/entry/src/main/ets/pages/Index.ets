import { LxAppNavigation } from 'lingxia';

@Entry
@Component
struct Index {
  aboutToAppear() { }

  build() {
    Column() {
      LxAppNavigation()
    }
    .width('100%')
    .height('100%')
  }
}
