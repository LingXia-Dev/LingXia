Page({
  data: {
    location: null,
    locationError: "",
    isLoading: false,
  },

  onLoad: function (options) {
    console.log("Location page onLoad options:", options);
  },

  getLocation: async function () {
    if (this.data.isLoading) {
      return;
    }

    this.setData({
      isLoading: true,
      locationError: "",
    });

    try {
      const location = await lx.getLocation({ highAccuracyExpireTime: 10_000 });

      console.log("Location info:", location);

      this.setData({
        location: location,
        isLoading: false,
      });
    } catch (error) {
      console.warn("Failed to get location:", error);

      this.setData({
        locationError: error instanceof Error ? error.message : String(error),
        isLoading: false,
      });
    }
  },

  // Clear location data
  clearLocation: async function () {
    this.setData({
      location: null,
      locationError: "",
    });
  },
});
