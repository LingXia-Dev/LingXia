Page({
  data: {
    coffee: undefined,
    location: undefined,
    time: ['09', '30'],
  },

  onLoad: function (options) {
    console.log('Picker page onLoad options:', options);
  },

  setCoffee: function (params) {
    const { value } = params;
    console.log('setCoffee:', value);
    this.setData({ coffee: value });
  },

  setLocation: function (params) {
    const { value } = params;
    console.log('setLocation:', value);
    this.setData({ location: value });
  },

  setTime: function (params) {
    const { value } = params;
    console.log('setTime:', value);
    this.setData({ time: value });
  },
});
