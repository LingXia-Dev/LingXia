const app = getApp();
const globalData = app.globalData;

Page({
  data: {
    greeting: globalData.greeting,
    imageUrl: "../../images/futuristic.jpg",
    ipAddr: globalData.ipAddr,
    greetCount: 0,
  },

  onReady: function () {
    // Add callback directly to App
    app.ipReadyCallback = (ip) => {
      console.log("IP received in Page:", ip);
      this.setData({
        ipAddr: ip,
      });
    };

    // Check if IP is already available
    if (app.globalData.ipAddr) {
      this.setData({
        ipAddr: app.globalData.ipAddr,
      });
    }
  },

  onUnload: function () {
    console.log("onUnload: -----");
  },

  onLoad: async function () {
    console.log("onLoad: ------");
    console.log(lx.env.USER_CACHE_PATH);
    let testFile = `${lx.env.USER_CACHE_PATH}/testFile`;
    const testContent = "Hello, World!";
    await Rong.writeTextFile(testFile, testContent);
    try {
      await Rong.mkdir(
        `/data/storage/el2/base/cache/lingxia/usercache/testminiapp`,
        {
          recursive: true,
        },
      );
    } catch (e) {
      console.log(e);
    }
  },

  onHide: function () {
    console.log("onHide: ------");
  },

  onShow: function () {
    console.log("onShow: +++++++");
  },

  greet: async function (option) {
    const count = this.data.greetCount + 1;
    await this.setData(
      {
        greeting: `👋 Hello ${option.name}! (#${count})

🌍 Greetings from appservice powered by Rust and JS engine
🕒 ${new Date().toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit" })}`,
        greetCount: count,
      },
      () => {
        console.log("setData callback");
      },
    );
  },
});

// futuristic.png is got from link below
// https://unsplash.com/photos/a-red-cube-surrounded-by-yellow-squares-on-a-blue-background-0PePVHSlu_8
