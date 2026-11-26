import { definePage, lx, type LocationInfo } from '@lingxia/logic';
import { failure, type Failure } from "../../shared/lib/errors";
export default definePage({
  data: {
    location: null as LocationInfo | null,
    locationError: null as Failure | null,
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
          d.locationError = null;
        });

        try {
          const location = await lx.device.location({ signal: AbortSignal.timeout(10_000) });

          console.log("Location info:", location);

          page.update(d => {
            d.location = location;
            d.isLoading = false;
          });
        } catch (error) {
          console.warn("Failed to get location:", error);

          page.update(d => {
            d.locationError = failure(error);
            d.isLoading = false;
          });
        }
      },
      // Clear location data
      async clearLocation() {
        page.update(d => {
          d.location = null;
          d.locationError = null;
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
