/// <reference path="./generated/logic-web.d.ts" />
// What a Logic program sees without importing. A View program never lists this entry, so `lx`
// there is "cannot find name"; in Logic it is a type whose only member names the import.
import type { AppConfig, AppLifecycleName, NoLifecycleTypos } from './index.js';

declare global {
  /** Import it: `import { lx } from '@lingxia/logic'`. */
  const lx: { readonly "import { lx } from '@lingxia/logic'": never };

  /**
   * `TCustom` is inferred from the members you write beside the lifecycle
   * hooks, so `this.myMethod()` resolves inside the config and a key that is
   * only a case away from a hook is rejected.
   */
  function App<TCustom>(
    config: AppConfig &
      TCustom &
      NoLifecycleTypos<TCustom, AppLifecycleName> &
      ThisType<AppConfig & TCustom>
  ): AppConfig & TCustom;
}
