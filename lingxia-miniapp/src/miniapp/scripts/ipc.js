(function () {
  "use strict"; // Enable strict mode

  const LOG_PREFIX = "[LingXia IPC]";

  // --- Core IPC State ---
  let nativePort = null; // The MessagePort received from native
  let hasMessageChannelListener = false; // Ensure listener is added only once
  const messageHandlers = []; // Callbacks registered via onNativeMessage
  const _messageQueue = []; // Add message queue
  let isProcessingQueue = false; // Add processing flag

  /**
   * Logs messages with a consistent prefix.
   * Uses console.log for info, console.error for errors.
   */
  function log(level, ...args) {
    const logger = level === "error" ? console.error : console.log;
    logger(LOG_PREFIX, ...args);
  }

  /**
   * Sends a message to the native host via the established MessagePort.
   * Handles potential errors during stringification or posting.
   * @param {object} message - The message object to send.
   */
  function postMessageToNative(message) {
    if (!nativePort) {
      log("error", "Cannot send message - MessagePort not initialized.");
      return;
    }
    try {
      nativePort.postMessage(JSON.stringify(message)); // Ensure we send a string
    } catch (e) {
      log("error", "Failed to send message to native:", e, message);
    }
  }

  /**
   * Processes the message queue, sending messages to registered handlers.
   */
  function processMessageQueue() {
    if (
      isProcessingQueue ||
      messageHandlers.length === 0 ||
      _messageQueue.length === 0
    ) {
      return;
    }

    isProcessingQueue = true;
    log(
      "info",
      `Processing ${_messageQueue.length} queued messages for ${messageHandlers.length} handlers.`,
    );

    // Process messages FIFO
    while (_messageQueue.length > 0) {
      const queuedMessage = _messageQueue.shift(); // Get first message
      if (queuedMessage) {
        messageHandlers.forEach((handler) => {
          try {
            handler(queuedMessage);
          } catch (e) {
            log("error", "Error executing message handler from queue:", e);
          }
        });
      }
    }

    isProcessingQueue = false;
  }

  /**
   * Handles incoming messages from the native host via the MessagePort.
   * Queues the message and attempts to process the queue.
   */
  function handleNativeMessage(event) {
    log("info", "Received message from native port (queueing):", event.data);
    if (!event.data) {
      log("warn", "Received empty message data from native.");
      return;
    }

    let parsedData;
    try {
      parsedData = JSON.parse(event.data); // Use this if native sends JSON strings
      log("info", "Parsed message data (queued):", parsedData);
      _messageQueue.push(parsedData); // Add to queue
      processMessageQueue(); // Attempt to process queue immediately
    } catch (e) {
      log("error", "Failed to parse message from native:", e, event.data);
      return;
    }
  }

  /**
   * Sets up the main MessageChannel listener on the window.
   * This listener waits for the native host to send the MessagePort.
   */
  function setupMessageChannelListener() {
    if (hasMessageChannelListener) {
      log("info", "MessageChannel listener already initialized.");
      return;
    }

    window.addEventListener("message", (event) => {
      // Check if it's the event containing the MessagePort
      if (event.ports && event.ports.length > 0) {
        if (nativePort) {
          log("warn", "Received new MessagePort, closing the old one.");
          nativePort.close();
        }

        nativePort = event.ports[0];
        log("info", "Received MessagePort from native.");

        // Setup the handler for messages coming FROM native on this port
        nativePort.onmessage = handleNativeMessage;

        // Start the port to allow messages to flow
        nativePort.start();

        // *** IMPORTANT: Signal readiness to native ***
        postMessageToNative({ type: "ipcReady", status: "initialized" });
      } else {
        log("info", "Ignoring window message event without ports.");
      }
    });

    hasMessageChannelListener = true;
    log("info", "MessageChannel listener initialized.");
  }

  // Create the lingxia interface if it doesn't exist
  if (!window.lingxia) {
    window.lingxia = {
      /**
       * Sends a message TO the native host.
       * @param {object} message - The message object to send.
       */
      postMessage: function (message) {
        postMessageToNative(message);
      },

      /**
       * Registers a callback function and processes any queued messages.
       */
      onNativeMessage: function (callback) {
        if (typeof callback === "function") {
          log("info", "Registering native message handler.");
          if (!messageHandlers.includes(callback)) {
            // Avoid duplicates
            messageHandlers.push(callback);
            log(
              "info",
              `Handler registered. Total handlers: ${messageHandlers.length}. Processing queue...`,
            );
            processMessageQueue(); // Process queue immediately after adding handler
          } else {
            log("warn", "Attempted to register duplicate message handler.");
          }
        } else {
          log("error", "Invalid callback provided to onNativeMessage.");
        }
      },

      // Expose internal state for debugging? (Optional)
      _getInternalState: function () {
        return {
          portReady: !!nativePort,
          listenerReady: hasMessageChannelListener,
          handlerCount: messageHandlers.length,
          queueSize: _messageQueue.length,
        };
      },
    };
    log("info", "window.lingxia interface created.");

    // Dispatch the ready event AFTER the interface is created
    // Use setTimeout to ensure it fires after current execution context
    setTimeout(() => {
      log("info", "Dispatching 'lingxiaready' event.");
      window.dispatchEvent(new CustomEvent("lingxiaready"));
    }, 0);
  } else {
    log("warn", "window.lingxia interface already exists.");
  }

  setupMessageChannelListener();

  log("info", "IPC script execution finished.");
})();
