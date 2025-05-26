App({
  onLaunch: async function () {
    try {
      const response = await fetch("https://api64.ipify.org?format=json");
      const data = await response.json();
      this.globalData.ipAddr = data.ip;
      console.log("Got public address: ", data.ip);
    } catch (error) {
      this.globalData.ipAddr = error.message;
    }

    // Call the registered callback function if available
    if (this.ipReadyCallback) {
      console.log("Calling IP ready callback");
      this.ipReadyCallback(this.globalData.ipAddr);
    }
  },

  globalData: {
    greeting: "This is from App's globalData.data",
    ipAddr: "loading",
  },
});
