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
          await lx.navigateTo({ page, query });
          return;
        }
        await lx.navigateTo({ page });
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
