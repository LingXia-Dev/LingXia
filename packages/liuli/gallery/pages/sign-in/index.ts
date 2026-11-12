Page({
  // Stands in for a real sign-in request, so the form shows its busy state.
  async signIn() {
    await new Promise(resolve => setTimeout(resolve, 1200));
  },
});
