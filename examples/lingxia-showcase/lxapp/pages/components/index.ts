import type { ConfiguredPageName, PageQuery } from "@lingxia/types";
import { definePage } from '@lingxia/types/page';

export default definePage({
  data: {},
  setup() {
    const actions = {
      async navigateTo(params: { page?: ConfiguredPageName; query?: PageQuery } = {}) {
        const { page, query } = params;
        if (!page) {
          return;
        }
        if (query && typeof query === "object") {
          await lx.nav.go({ page, params: query }).opened;
          return;
        }
        await lx.nav.go({ page }).opened;
      },
    };
    return {
      onLoad(options) {
        console.log("Components page onLoad options:", options);
      },
      actions,
    };
  },
});
