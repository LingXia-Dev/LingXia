(function () {
  const NATIVE_HANDLER_NAME = "LingXia";
  const GLOBAL_RECEIVER_NAME = "__LingXiaRecvMessage";
  const CALL_TIMEOUT_MS = 5000;
  const LOG_PREFIX = "[LX.Bridge]";
  const MESSAGE_PORT_TYPE = "messageport";

  // Simple debug flags
  const debugFlags = {
    data: false, // setData operations
    proto: false, // protocol messages
    all: false, // enable all debugging
  };

  // Check if specific debug flag is enabled
  function isDebugEnabled(flag) {
    return debugFlags.all || debugFlags[flag];
  }

  // Safe JSON stringify that handles circular references
  function _safeStringify(obj, space) {
    const seen = new WeakSet();
    return JSON.stringify(
      obj,
      (key, value) => {
        if (typeof value === "object" && value !== null) {
          if (seen.has(value)) {
            return "[Circular Reference]";
          }
          seen.add(value);
        }
        return value;
      },
      space,
    );
  }

  let messageCounter = 0;
  const pendingCalls = new Map(); // msgId -> { resolve, reject, timerId }
  let pageData = {};
  const dataSubscribers = new Set();
  const subscriberInitStatus = new WeakMap();
  let messagePort = null; // Unified port for MessagePort-based platforms

  // Detect communication method based on available APIs
  // Communication method is provided by host
  // Expected values: 'webkit' (iOS/macOS), 'messageport' (Android/Harmony)
  const communicationMethod =
    (typeof window !== "undefined" && window.__LX_BRIDGE_METHOD) || "unknown";

  function log(...args) {
    console.log(LOG_PREFIX, ...args);
  }
  function warn(...args) {
    console.warn(LOG_PREFIX, ...args);
  }
  function error(...args) {
    console.error(LOG_PREFIX, ...args);
  }

  // Creates an isolated copy of data
  function _deepCopy(data) {
    try {
      if (typeof structuredClone === "function") {
        return structuredClone(data);
      } else {
        return JSON.parse(JSON.stringify(data));
      }
    } catch (e) {
      error("Failed to deep copy data:", e);
      return {};
    }
  }

  // Set value by path in object
  function _setValueByPath(obj, path, value) {
    if (
      typeof path !== "string" ||
      path === "" ||
      typeof obj !== "object" ||
      obj === null
    ) {
      return false;
    }

    const parts = path.replace(/\[(\d+)\]/g, ".$1").split(".");
    let current = obj;

    for (let i = 0; i < parts.length - 1; i++) {
      const key = parts[i];
      const nextKey = parts[i + 1];
      const isNextKeyArrayIndex = /^\d+$/.test(nextKey);

      if (current[key] === undefined || current[key] === null) {
        current[key] = isNextKeyArrayIndex ? [] : {};
      } else if (typeof current[key] !== "object") {
        current[key] = isNextKeyArrayIndex ? [] : {};
      } else if (isNextKeyArrayIndex && !Array.isArray(current[key])) {
        current[key] = [];
      }
      current = current[key];
      if (typeof current !== "object" || current === null) {
        return false;
      }
    }

    const finalKey = parts[parts.length - 1];
    current[finalKey] = value;
    return true;
  }

  // Delete value by path in object
  function _deleteValueByPath(obj, path) {
    if (
      typeof path !== "string" ||
      path === "" ||
      typeof obj !== "object" ||
      obj === null
    ) {
      return false;
    }

    const parts = path.replace(/\[(\d+)\]/g, ".$1").split(".");
    let current = obj;

    for (let i = 0; i < parts.length - 1; i++) {
      const key = parts[i];
      if (typeof current[key] !== "object" || current[key] === null) {
        return false;
      }
      current = current[key];
    }

    const finalKey = parts[parts.length - 1];
    if (Array.isArray(current)) {
      const index = parseInt(finalKey, 10);
      if (!isNaN(index) && index >= 0 && index < current.length) {
        current.splice(index, 1);
        return true;
      }
    } else if (typeof current === "object") {
      delete current[finalKey];
      return true;
    }
    return false;
  }

  // Applies a patch object to a target object
  function _applyPatch(target, patch) {
    if (
      typeof target !== "object" ||
      target === null ||
      typeof patch !== "object" ||
      patch === null
    ) {
      return patch;
    }

    let changesApplied = false;
    for (const path in patch) {
      if (Object.prototype.hasOwnProperty.call(patch, path)) {
        const value = patch[path];
        if (value === undefined) {
          if (_deleteValueByPath(target, path)) changesApplied = true;
        } else {
          if (_setValueByPath(target, path, value)) changesApplied = true;
        }
      }
    }
    return changesApplied ? patch : {};
  }

  // Send message to native layer
  function _sendMessageToNative(message) {
    if (isDebugEnabled("proto")) {
      console.log("→", JSON.stringify(message, null, 2));
    }
    try {
      if (communicationMethod === "webkit") {
        window.webkit.messageHandlers[NATIVE_HANDLER_NAME].postMessage(message);
      } else if (communicationMethod === MESSAGE_PORT_TYPE && messagePort) {
        const messageString = _safeStringify(message);
        messagePort.postMessage(messageString);
      } else {
        warn("Bridge not ready for sending");
      }
    } catch (e) {
      error("Send message error:", e, message);
    }
  }

  // Get MessagePort using proxy mechanism
  function _getMessagePort() {
    return new Promise((resolve) => {
      // Trigger native to send LingXiaPort
      window.LingXiaProxy.getPort("LingXiaPort");

      // Wait for port init event
      const handlePortInit = (event) => {
        if (event.data === "LingXia-port-init") {
          window.removeEventListener("message", handlePortInit);
          const port = event.ports[0];

          // Connect the port
          LingXiaBridge._connectWebMessagePort(port);
          resolve(port);
        }
      };

      window.addEventListener("message", handlePortInit);
    });
  }

  // Process incoming messages
  function _handleIncomingMessage(message) {
    if (isDebugEnabled("proto")) {
      console.log("←", JSON.stringify(message, null, 2));
    }
    if (!message || typeof message !== "object" || !message.type) {
      warn("Invalid message format:", message);
      return;
    }

    switch (message.type) {
      case "reply":
        _handleReply(message);
        break;
      case "event":
        _handleEvent(message);
        break;
      default:
        warn("Unknown message type:", message.type);
    }
  }

  // Handle reply from native
  function _handleReply(replyMessage) {
    const msgId = replyMessage.msgId;
    if (!msgId || !pendingCalls.has(msgId)) {
      warn("Reply for unknown msgId:", replyMessage);
      return;
    }

    const callInfo = pendingCalls.get(msgId);
    pendingCalls.delete(msgId);
    clearTimeout(callInfo.timerId);

    try {
      if (replyMessage.payload?.success === true) {
        if (replyMessage.payload.hasOwnProperty("result")) {
          callInfo.resolve(replyMessage.payload.result);
        } else {
          callInfo.resolve();
        }
      } else if (replyMessage.payload?.success === false) {
        callInfo.reject(
          replyMessage.payload.error || { message: "Unknown error" },
        );
      } else {
        callInfo.reject({ message: "Invalid reply payload" });
      }
    } catch (e) {
      error("Reply processing error:", e);
    }
  }

  // Handle event from native
  function _handleEvent(eventMessage) {
    const { name, payload } = eventMessage;

    if (name === "setData") {
      let dataToApply;
      let callbackId = null;

      if (payload && typeof payload.data !== "undefined") {
        dataToApply = payload.data;
        callbackId = payload.callbackId;
      } else {
        dataToApply = payload;
      }

      // Debug logging before applying patch
      if (isDebugEnabled("data")) {
        console.group("[LingXia Debug] setData Update");
        console.log("Previous data:", JSON.parse(_safeStringify(pageData)));
        console.log("Applying patch:", dataToApply);
      }

      _applyPatch(pageData, dataToApply);

      // Debug logging after applying patch
      if (isDebugEnabled("data")) {
        console.log("Updated data:", JSON.parse(_safeStringify(pageData)));
        console.log("Active subscribers:", dataSubscribers.size);
        console.groupEnd();
      }

      // Notify subscribers immediately
      dataSubscribers.forEach((listener) => {
        try {
          if (!subscriberInitStatus.has(listener)) {
            subscriberInitStatus.set(listener, true);
            listener(pageData, null, true); // Initial data: (data, callbackId=null, isInitialData=true)
          } else {
            listener(pageData, callbackId, false); // Update data: (data, callbackId, isInitialData=false)
          }
        } catch (e) {
          warn("Data subscriber error:", e);
        }
      });

      // Send callback automatically if provided (maintain original behavior)
      if (callbackId) {
        _sendCallback(callbackId);
      }
    } else if (name === "samelevel") {
      handleSameLevelEvent(payload);
    } else {
      warn("Unknown event:", name);
    }
  }

  // Send callback to native
  function _sendCallback(callbackId) {
    _sendMessageToNative({
      msgId: null,
      type: "callback",
      callbackId: callbackId,
    });
  }

  const LingXiaBridge = {
    /**
     * Call a function in the Logic Layer.
     * @param {string} name - Function name.
     * @param {object|null} payload - Function arguments.
     * @returns {Promise<void>}
     */
    call: function (name, payload = null) {
      return new Promise((resolve, reject) => {
        const msgId = `view-${Date.now()}-${messageCounter++}`;
        const timerId = setTimeout(() => {
          if (pendingCalls.has(msgId)) {
            pendingCalls
              .get(msgId)
              .reject({ message: `Call '${name}' timed out` });
            pendingCalls.delete(msgId);
          }
        }, CALL_TIMEOUT_MS);

        pendingCalls.set(msgId, { resolve, reject, timerId });
        _sendMessageToNative({
          msgId: msgId,
          type: "call",
          name: name,
          payload: payload,
        });
      });
    },

    /**
     * Send a fire-and-forget event to the Logic Layer.
     * @param {string} name - Event name.
     * @param {object|null} payload - Event data.
     */
    event: function (name, payload = null) {
      _sendMessageToNative({
        msgId: null,
        type: "event",
        name: name,
        payload: payload,
      });
    },

    /**
     * Subscribe to page data updates from the Logic Layer.
     * @param {function} callback - Function called on data updates with params:
     *   - data: Object (complete data on first call, patch data on updates)
     *   - callbackId: String|null (callback ID if provided by native layer)
     *   - isInitialData: Boolean (true for initial data, false for updates)
     * @returns {function} Unsubscribe function
     */
    subscribe: function (callback) {
      if (typeof callback !== "function") {
        error("Subscriber must be a function");
        return () => {};
      }

      dataSubscribers.add(callback);

      // Send initial data immediately if available
      if (Object.keys(pageData).length > 0) {
        if (dataSubscribers.has(callback)) {
          subscriberInitStatus.set(callback, true);
          try {
            callback(_deepCopy(pageData), null, true);
          } catch (e) {
            error("Initial data callback error:", e);
          }
        }
      }

      return () => {
        dataSubscribers.delete(callback);
        subscriberInitStatus.delete(callback);
      };
    },

    // Connect to WebMessage port (used by MessagePort-based platforms)
    _connectWebMessagePort: function (port) {
      if (communicationMethod !== MESSAGE_PORT_TYPE) return;

      log("Connecting WebMessage port...");

      // Store the unified port
      messagePort = port;

      // Set up message handler
      port.onmessage = (event) => {
        let messageData = event.data;
        if (typeof messageData === "string") {
          try {
            messageData = JSON.parse(messageData);
          } catch (e) {
            error("Invalid JSON from MessagePort:", e);
            return;
          }
        }
        _handleIncomingMessage(messageData);
      };

      log("MessagePort connected and ready");
      this.event("LXPortRdy");
    },

    // Internal: Receive message from evaluate_javascript (WebKit platforms)
    _receiveEvaluateMessage: function (messageString) {
      if (communicationMethod !== "webkit") return;
      try {
        if (!messageString) return;
        const message = JSON.parse(messageString);
        _handleIncomingMessage(message);
      } catch (e) {
        error("Invalid JSON from evaluate_javascript:", e);
      }
    },

    // Debug control with property setters
    debug: new Proxy(debugFlags, {
      get(target, prop) {
        return target[prop];
      },
      set(target, prop, value) {
        if (prop in target) {
          target[prop] = !!value;
          console.log(
            `[LingXia Debug] ${prop} debugging ${value ? "enabled" : "disabled"}`,
          );
          return true;
        }
        return false;
      },
    }),
  };

  // Create lx proxy object for API interception
  const lx = new Proxy(
    {},
    {
      get: function (_target, prop, _receiver) {
        // Return a function that will call the native layer
        return function (...args) {
          // Method parameters should be either empty or a single object
          let payload = null;
          if (
            args.length === 1 &&
            typeof args[0] === "object" &&
            args[0] !== null
          ) {
            payload = args[0];
          } else if (args.length > 1) {
            warn(
              `lx.${prop} called with multiple arguments, only the first object argument will be used`,
            );
            if (typeof args[0] === "object" && args[0] !== null) {
              payload = args[0];
            }
          }

          // Call the native layer using LingXiaBridge with "lx." prefix to avoid name conflicts
          return LingXiaBridge.call(`lx.${prop}`, payload);
        };
      },
    },
  );

  window.lx = lx;

  // Initialize the bridge when DOM is ready
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", _init);
  } else {
    _init();
  }

  function _init() {
    log(`Detected communication method: ${communicationMethod}`);

    // Platform-specific initialization
    if (communicationMethod === "webkit") {
      window[GLOBAL_RECEIVER_NAME] = LingXiaBridge._receiveEvaluateMessage;
      LingXiaBridge.event("LXPortRdy");
    } else if (communicationMethod === MESSAGE_PORT_TYPE) {
      _getMessagePort().catch((e) => {
        warn("Failed to initialize MessagePort:", e);
      });
    } else {
      warn("Unknown communication method, bridge may not work properly");
    }

    log("LingXia Bridge initialization completed");
  }

  window.LingXiaBridge = LingXiaBridge;

  function handleSameLevelEvent(msg) {
    try {
      const message =
        typeof msg === "string"
          ? JSON.parse(msg)
          : msg && typeof msg === "object"
            ? msg
            : null;
      if (!message || !message.id) {
        warn("SameLevel receive: invalid message", msg);
        return;
      }
      if (message.action !== "component.event") return;

      const handler = sameLevelHandlers.get(message.id);
      if (typeof handler !== "function") return;
      if (isDebugEnabled("proto")) {
        console.log("[SameLevel] ← native:", message);
      }
      handler(message);
    } catch (e) {
      error("SameLevel receive error:", e);
    }
  }

  // Global registry for native-backed components
  const sameLevelHandlers = new Map();
  const sameLevelQueue = [];
  let sameLevelReady = false;

  function hasSameLevelHandler() {
    if (typeof window === "undefined") return false;

    if (
      window.webkit &&
      window.webkit.messageHandlers &&
      window.webkit.messageHandlers.SameLevel
    ) {
      return true;
    }

    // Android/HarmonyOS: JavaScriptInterface proxy
    if (
      window.SameLevelNative &&
      typeof window.SameLevelNative.postMessage === "function"
    ) {
      return true;
    }

    return false;
  }

  function _postSameLevelMessage(message) {
    if (
      window.webkit &&
      window.webkit.messageHandlers &&
      window.webkit.messageHandlers.SameLevel
    ) {
      window.webkit.messageHandlers.SameLevel.postMessage(message);
      return;
    }

    if (
      window.SameLevelNative &&
      typeof window.SameLevelNative.postMessage === "function"
    ) {
      const msgString = _safeStringify(message);
      window.SameLevelNative.postMessage(msgString);
      return;
    }
  }

  function flushSameLevelQueue() {
    if (!hasSameLevelHandler() || sameLevelQueue.length === 0) return;
    sameLevelReady = true;
    while (sameLevelQueue.length) {
      const msg = sameLevelQueue.shift();
      try {
        if (isDebugEnabled("proto")) {
          console.log("[SameLevel] flush → native:", msg);
        }
        _postSameLevelMessage(msg);
      } catch (e) {
        error("Failed to flush SameLevel message:", e);
        break;
      }
    }
  }

  function sendSameLevelMessage(message) {
    try {
      const hasHandler = hasSameLevelHandler();
      if (!hasHandler) {
        console.log("[SameLevel] queue message (no handler yet):", message);
        sameLevelQueue.push(message);
        return;
      }
      if (!sameLevelReady) {
        flushSameLevelQueue();
      }
      if (isDebugEnabled("proto")) {
        console.log("[SameLevel] → native:", message);
      }
      _postSameLevelMessage(message);
    } catch (e) {
      error("Failed to send SameLevel message:", e);
    }
  }

  // Expose SameLevel helpers for external packages (e.g., @lingxia/view)
  LingXiaBridge.sameLevel = {
    send: sendSameLevelMessage,
    hasHandler: hasSameLevelHandler,
    flush: flushSameLevelQueue,
    register(id, handler) {
      if (!id || typeof handler !== "function") return () => {};
      sameLevelHandlers.set(id, handler);
      return () => {
        sameLevelHandlers.delete(id);
      };
    },
    unregister(id) {
      sameLevelHandlers.delete(id);
    },
  };
})();
