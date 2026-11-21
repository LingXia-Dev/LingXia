import type { LocationInfo } from "@lingxia/types";
import { definePage } from '@lingxia/types/page';
export default definePage({
  data: {
    location: null as LocationInfo | null,
    locationError: "",
    isLoading: false,
  },
  setup(page) {
    const actions = {
      async getLocation() {
        if (page.data.isLoading) {
          return;
        }

        page.update(d => {
          d.isLoading = true;
          d.locationError = "";
        });

        try {
          const location = await lx.device.location({ timeoutMs: 10_000 });

          console.log("Location info:", location);

          page.update(d => {
            d.location = location;
            d.isLoading = false;
          });
        } catch (error) {
          console.warn("Failed to get location:", error);

          page.update(d => {
            d.locationError = error instanceof Error ? error.message : String(error);
            d.isLoading = false;
          });
        }
      },
      // Clear location data
      async clearLocation() {
        page.update(d => {
          d.location = null;
          d.locationError = "";
        });
      },
    };
    return {
      onLoad(options) {
        console.log("Location page onLoad options:", options);
      },
      actions,
    };
  },
});
