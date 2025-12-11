Page({
  data: {
    greeting: 'Welcome to your LingXia app',
    greetCount: 0
  },

  greet({ name }) {
    const count = this.data.greetCount + 1;
    this.setData({
      greetCount: count,
      greeting: `Hello ${name}! (#${count})`
    });
  }
});
