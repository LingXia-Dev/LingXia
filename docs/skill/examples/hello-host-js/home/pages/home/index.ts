// Logic for the embedded home lxapp. Identical shape to ../hello-lxapp.
// The host shell opens this page via ui.launch.initialSurface -> surface
// -> content.appId -> lxapp.json.pages[0].

interface HomeData {
  count: number;
}

Page<HomeData>({
  data: { count: 0 },
  increment() {
    this.setData({ count: this.data.count + 1 });
  },
});
