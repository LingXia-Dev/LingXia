import type { HomePage } from './contract';

Page<HomePage['data'], HomePage['actions']>({
  data: {
    greeting: '',
    greetCount: 0
  },

  async greet(payload: { name: string }) {
    const count = this.data.greetCount + 1;
    this.setData({
      greetCount: count,
      greeting: `Hello, ${payload.name}! (#${count})`
    });
  }
});
