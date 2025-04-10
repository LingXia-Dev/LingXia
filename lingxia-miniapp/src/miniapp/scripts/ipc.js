(function () {
  // Skip if already initialized
  if (window.lingxia) {
    return;
  }

  // Create the lingxia interface
  window.lingxia = {
    postMessage: function (message) {
      if (!window._port) {
        console.error("[LingXia] MessagePort not initialized");
        return;
      }
      try {
        window._port.postMessage(JSON.stringify(message));
      } catch (e) {
        console.error("[LingXia] Failed to send message:", e);
      }
    },
  };

  // Set up message channel
  window.addEventListener("message", function (event) {
    if (!event.ports || !event.ports.length) {
      return;
    }

    const port = event.ports[0];
    port.onmessage = function (event) {
      if (!event.data) {
        return;
      }

      try {
        const data = JSON.parse(event.data);
        console.log("[LingXia] Received message:", data);
      } catch (e) {
        console.error("[LingXia] Failed to parse message:", e);
      }
    };

    window._port = port;
    port.start();
  });
})();
