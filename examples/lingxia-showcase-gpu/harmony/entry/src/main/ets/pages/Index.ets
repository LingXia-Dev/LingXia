import { LxAppNavigation, handleSystemBack } from 'lingxia';

@Entry
@Component
struct Index {
  aboutToAppear() { }

  onBackPress(): boolean {
    return handleSystemBack();
  }

  build() {
    Column() {
      LxAppNavigation()
    }
    .width('100%')
    .height('100%')
  }
}
