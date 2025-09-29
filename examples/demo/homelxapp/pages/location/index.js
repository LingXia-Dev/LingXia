Page({
  data: {
    location: null,
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
    });

    try {
      // Call location API without parameters (uses default WGS84)
      const location = await lx.getLocation();

      console.log("Location info:", location);

      this.setData({
        location: location,
        isLoading: false,
      });
    } catch (error) {
      console.error("Failed to get location:", error);

      this.setData({
        isLoading: false,
      });

      // Show error toast
      lx.showToast({
        title: `Location error: ${error.message}`,
        icon: "none",
        duration: 3000,
      });
    }
  },

  // Clear location data
  clearLocation: async function () {
    this.setData({
      location: null,
    });
  },
});
