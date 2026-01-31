Page({
  data: {
    greeting: '',
    greetCount: 0
  },

  greet: function(payload: { name: string }) {
    const count = this.data.greetCount + 1;
    const time = new Date().toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit'
    });
    this.setData({
      greetCount: count,
      greeting: 'Hello, ' + payload.name + '! 👋\nGreeting #' + count + ' at ' + time
    });
  }
});
