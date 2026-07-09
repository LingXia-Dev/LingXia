Page({
  data: {
    greeting: '',
    greetCount: 0
  },

  // --worker: the greeting now comes from the `hello` cloud function (mock
  // in dev, live in prod) instead of being computed locally. The View
  // (index.tsx/.vue) is unchanged — only this action's body calls lx.cloud.
  greet: async function(payload: { name: string }) {
    const { greeting } = await lx.cloud.invoke("hello", { name: payload.name });
    this.setData({ greeting });
  }
});
