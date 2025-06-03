(function () {
  const NATIVE_HANDLER_NAME = "LingXia";
  const GLOBAL_RECEIVER_NAME = "__LingXiaRecvMessage";
  const CALL_TIMEOUT_MS = 5000;
  const LOG_PREFIX = "[LingxiaBridge]";
  const ANDROID_PORT_INIT_CMD = "LingXia-port-init";

  let messageCounter = 0;
  const pendingCalls = new Map(); // msgId -> { resolve, reject, timerId }
  let pageData = {};
  const dataSubscribers = new Set();
  const subscriberInitStatus = new WeakMap();
  let androidMessagePort = null;

  const isIOS = !!(
    window.webkit &&
    window.webkit.messageHandlers &&
    window.webkit.messageHandlers[NATIVE_HANDLER_NAME]
  );
  const isAndroid = !isIOS;

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
    try {
      if (isIOS) {
        window.webkit.messageHandlers[NATIVE_HANDLER_NAME].postMessage(message);
      } else if (isAndroid && androidMessagePort) {
        const messageString = JSON.stringify(message);
        androidMessagePort.postMessage(messageString);
      } else {
        warn("Bridge not ready for sending");
      }
    } catch (e) {
      error("Send message error:", e, message);
    }
  }

  // Process incoming messages
  function _handleIncomingMessage(message) {
    log(
      "Message Received:",
      typeof message === "object" ? JSON.stringify(message) : message,
    );
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

    if (!name) {
      warn("Event missing name field:", eventMessage);
      return;
    }

    try {
      if (name === "setData") {
        let dataToApply;
        let callbackId = null;

        if (payload && typeof payload.data !== "undefined") {
          dataToApply = payload.data;
          callbackId = payload.callbackId;
        } else {
          dataToApply = payload;
        }

        const appliedPatch = _deepCopy(dataToApply);
        _applyPatch(pageData, dataToApply);

        // Notify subscribers immediately
        dataSubscribers.forEach((listener) => {
          try {
            if (!subscriberInitStatus.has(listener)) {
              subscriberInitStatus.set(listener, true);
              listener(_deepCopy(pageData), callbackId, true);
            } else {
              listener(appliedPatch, callbackId, false);
            }
          } catch (e) {
            error("Subscriber error:", e);
          }
        });

        // No reply needed for events
      } else {
        // Handle other events if needed
        log("Unhandled event:", name);
      }
    } catch (e) {
      error("Event handler error:", e);
    }
  }

  // Send reply to native
  function _sendReply(originalMsgId, success, errorPayload = null) {
    const replyPayload = success
      ? { success: true }
      : { success: false, error: errorPayload || { message: "Unknown error" } };

    _sendMessageToNative({
      msgId: originalMsgId,
      type: "reply",
      payload: replyPayload,
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

    /**
     * Get a deep copy of the current page data.
     * @returns {object}
     */
    getCurrentData: function () {
      return _deepCopy(pageData);
    },

    /**
     * Called by data subscribers (e.g., UI frameworks) after they have processed
     * a data update associated with a callbackId and wish to trigger the callback in the Logic Layer.
     * @param {string} callbackId - The ID originally provided with the setData call.
     */
    resolveCallback: function (callbackId) {
      if (typeof callbackId !== "string" || !callbackId) {
        error("Invalid callbackId");
        return;
      }

      _sendMessageToNative({
        msgId: null,
        type: "callback",
        callbackId: callbackId,
      });
    },

    // Connect to Android message port
    _connectAndroidPort: function (port) {
      if (!isAndroid) return;

      // Always use the new port. If there was an old one, it's implicitly replaced.
      // The native side is responsible for managing the lifecycle of its end of the previous port.
      androidMessagePort = port;

      androidMessagePort.onmessage = (event) => {
        let messageData = event.data;
        if (typeof messageData === "string") {
          try {
            messageData = JSON.parse(messageData);
          } catch (e) {
            error("Invalid JSON from Android Port:", e);
            return;
          }
        }
        _handleIncomingMessage(messageData);
      };

      try {
        this.event("LXPortRdy");
        log("Android port connected and ready");
      } catch (e) {
        error("Failed to send LXPortRdy:", e);
      }
    },

    // Internal: Receive iOS message
    _receiveIOsMessage: function (messageString) {
      if (!isIOS) return;
      try {
        if (!messageString) return;
        const message = JSON.parse(messageString);
        _handleIncomingMessage(message);
      } catch (e) {
        error("Invalid JSON from iOS:", e);
      }
    },
  };

  // Platform Initialization
  if (isIOS) {
    window[GLOBAL_RECEIVER_NAME] = LingXiaBridge._receiveIOsMessage;
    setTimeout(() => LingXiaBridge.event("LXPortRdy"), 0);
  }

  if (isAndroid) {
    window.addEventListener(
      "message",
      (event) => {
        if (
          event.data === ANDROID_PORT_INIT_CMD &&
          event.ports &&
          event.ports.length > 0
        ) {
          LingXiaBridge._connectAndroidPort(event.ports[0]);
        }
      },
      false,
    );
  }

  // Create lx proxy object for API interception
  const lx = new Proxy(
    {},
    {
      get: function (target, prop, receiver) {
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
  window.LingXiaBridge = LingXiaBridge;
  log("Lingxia Bridge initialized.");
})();
