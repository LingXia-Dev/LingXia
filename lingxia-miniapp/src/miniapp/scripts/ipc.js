(function () {
  if (!window.lingxia) {
    window.lingxia = {
      postMessage: function (message) {
        MiniApp.postMessage(message);
      },
    };
    console.log("MiniApp bridge initialized");
    window.lingxia.postMessage('{"type":"BRIDGE_READY"}');
    return true;
  }
  console.log("MiniApp bridge already exists");
  return false;
})();
