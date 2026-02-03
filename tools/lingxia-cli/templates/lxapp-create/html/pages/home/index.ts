Page({
  data: {
    greeting: '',
    greetCount: 0
  },

  greet(payload: { name: string }) {
    const count = this.data.greetCount + 1;
    this.setData({
      greetCount: count,
      greeting: `Hello, ${payload.name}! 👋 (#${count})`
    });
  }
});
