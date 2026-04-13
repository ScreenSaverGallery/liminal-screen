// ../node_modules/@tauri-apps/api/external/tslib/tslib.es6.js
function __classPrivateFieldGet(receiver, state, kind, f) {
  if (kind === "a" && !f)
    throw new TypeError("Private accessor was defined without a getter");
  if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver))
    throw new TypeError("Cannot read private member from an object whose class did not declare it");
  return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
}
function __classPrivateFieldSet(receiver, state, value, kind, f) {
  if (kind === "m")
    throw new TypeError("Private method is not writable");
  if (kind === "a" && !f)
    throw new TypeError("Private accessor was defined without a setter");
  if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver))
    throw new TypeError("Cannot write private member to an object whose class did not declare it");
  return kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value), value;
}

// ../node_modules/@tauri-apps/api/core.js
var _Channel_onmessage;
var _Channel_nextMessageIndex;
var _Channel_pendingMessages;
var _Channel_messageEndIndex;
var _Resource_rid;
var SERIALIZE_TO_IPC_FN = "__TAURI_TO_IPC_KEY__";
function transformCallback(callback, once = false) {
  return window.__TAURI_INTERNALS__.transformCallback(callback, once);
}

class Channel {
  constructor(onmessage) {
    _Channel_onmessage.set(this, undefined);
    _Channel_nextMessageIndex.set(this, 0);
    _Channel_pendingMessages.set(this, []);
    _Channel_messageEndIndex.set(this, undefined);
    __classPrivateFieldSet(this, _Channel_onmessage, onmessage || (() => {}), "f");
    this.id = transformCallback((rawMessage) => {
      const index = rawMessage.index;
      if ("end" in rawMessage) {
        if (index == __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")) {
          this.cleanupCallback();
        } else {
          __classPrivateFieldSet(this, _Channel_messageEndIndex, index, "f");
        }
        return;
      }
      const message = rawMessage.message;
      if (index == __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")) {
        __classPrivateFieldGet(this, _Channel_onmessage, "f").call(this, message);
        __classPrivateFieldSet(this, _Channel_nextMessageIndex, __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") + 1, "f");
        while (__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") in __classPrivateFieldGet(this, _Channel_pendingMessages, "f")) {
          const message2 = __classPrivateFieldGet(this, _Channel_pendingMessages, "f")[__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")];
          __classPrivateFieldGet(this, _Channel_onmessage, "f").call(this, message2);
          delete __classPrivateFieldGet(this, _Channel_pendingMessages, "f")[__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f")];
          __classPrivateFieldSet(this, _Channel_nextMessageIndex, __classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") + 1, "f");
        }
        if (__classPrivateFieldGet(this, _Channel_nextMessageIndex, "f") === __classPrivateFieldGet(this, _Channel_messageEndIndex, "f")) {
          this.cleanupCallback();
        }
      } else {
        __classPrivateFieldGet(this, _Channel_pendingMessages, "f")[index] = message;
      }
    });
  }
  cleanupCallback() {
    window.__TAURI_INTERNALS__.unregisterCallback(this.id);
  }
  set onmessage(handler) {
    __classPrivateFieldSet(this, _Channel_onmessage, handler, "f");
  }
  get onmessage() {
    return __classPrivateFieldGet(this, _Channel_onmessage, "f");
  }
  [(_Channel_onmessage = new WeakMap, _Channel_nextMessageIndex = new WeakMap, _Channel_pendingMessages = new WeakMap, _Channel_messageEndIndex = new WeakMap, SERIALIZE_TO_IPC_FN)]() {
    return `__CHANNEL__:${this.id}`;
  }
  toJSON() {
    return this[SERIALIZE_TO_IPC_FN]();
  }
}
async function invoke(cmd, args = {}, options) {
  return window.__TAURI_INTERNALS__.invoke(cmd, args, options);
}
_Resource_rid = new WeakMap;

// ../node_modules/@tauri-apps/api/event.js
var TauriEvent;
(function(TauriEvent2) {
  TauriEvent2["WINDOW_RESIZED"] = "tauri://resize";
  TauriEvent2["WINDOW_MOVED"] = "tauri://move";
  TauriEvent2["WINDOW_CLOSE_REQUESTED"] = "tauri://close-requested";
  TauriEvent2["WINDOW_DESTROYED"] = "tauri://destroyed";
  TauriEvent2["WINDOW_FOCUS"] = "tauri://focus";
  TauriEvent2["WINDOW_BLUR"] = "tauri://blur";
  TauriEvent2["WINDOW_SCALE_FACTOR_CHANGED"] = "tauri://scale-change";
  TauriEvent2["WINDOW_THEME_CHANGED"] = "tauri://theme-changed";
  TauriEvent2["WINDOW_CREATED"] = "tauri://window-created";
  TauriEvent2["WEBVIEW_CREATED"] = "tauri://webview-created";
  TauriEvent2["DRAG_ENTER"] = "tauri://drag-enter";
  TauriEvent2["DRAG_OVER"] = "tauri://drag-over";
  TauriEvent2["DRAG_DROP"] = "tauri://drag-drop";
  TauriEvent2["DRAG_LEAVE"] = "tauri://drag-leave";
})(TauriEvent || (TauriEvent = {}));
async function _unlisten(event, eventId) {
  window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(event, eventId);
  await invoke("plugin:event|unlisten", {
    event,
    eventId
  });
}
async function listen(event, handler, options) {
  var _a;
  const target = typeof (options === null || options === undefined ? undefined : options.target) === "string" ? { kind: "AnyLabel", label: options.target } : (_a = options === null || options === undefined ? undefined : options.target) !== null && _a !== undefined ? _a : { kind: "Any" };
  return invoke("plugin:event|listen", {
    event,
    target,
    handler: transformCallback(handler)
  }).then((eventId) => {
    return async () => _unlisten(event, eventId);
  });
}

// app/remote-options/remote-options.ts
var startsInInput = null;
var displayOffInput = null;
var runOnBatteryInput = null;
var debugInput = null;
var saverUrlDisplay = null;
var idleTimeElement = null;
var statusTextElement = null;
var statusDotElement = null;
async function init() {
  console.log("Remote Options - Initializing...");
  try {
    cacheUIElements();
    setupEventListeners();
    await loadOptions();
    setupIPCListeners();
    console.log("Remote Options - Initialized successfully");
  } catch (error) {
    console.error("Failed to initialize remote options:", error);
  }
}
function cacheUIElements() {
  startsInInput = document.getElementById("starts-in");
  displayOffInput = document.getElementById("display-off");
  runOnBatteryInput = document.getElementById("run-on-battery");
  debugInput = document.getElementById("debug-mode");
  saverUrlDisplay = document.getElementById("saver-url-display");
  idleTimeElement = document.getElementById("idle-time");
  statusTextElement = document.getElementById("status-text");
  statusDotElement = document.querySelector(".status-dot");
}
function setupEventListeners() {
  const saveBtn = document.getElementById("save-btn");
  if (saveBtn) {
    saveBtn.addEventListener("click", async () => {
      console.log("Save button clicked");
      await saveOptions();
    });
  }
  const previewBtn = document.getElementById("preview-btn");
  if (previewBtn) {
    previewBtn.addEventListener("click", async () => {
      console.log("Preview button clicked");
      await previewScreensaver();
    });
  }
  const resetBtn = document.getElementById("reset-btn");
  if (resetBtn) {
    resetBtn.addEventListener("click", async () => {
      console.log("Reset button clicked");
      if (confirm("Reset all options to defaults?")) {
        await resetOptions();
      }
    });
  }
}
function setupIPCListeners() {
  listen("options-updated", async (event) => {
    console.log("Options updated from main app:", event.payload);
    await loadOptions();
  });
}
async function loadOptions() {
  try {
    const options = await invoke("get_options");
    console.log("Loaded options:", options);
    loadOptionsIntoForm(options);
    updateOptionsDisplay(options);
  } catch (error) {
    console.error("Failed to load options:", error);
  }
}
function loadOptionsIntoForm(options) {
  if (startsInInput) {
    startsInInput.value = String(options.starts_in || 0.2);
  }
  if (displayOffInput) {
    displayOffInput.value = String(options.display_off_in || 1);
  }
  if (runOnBatteryInput) {
    runOnBatteryInput.checked = options.run_on_battery || false;
  }
  if (debugInput) {
    debugInput.checked = options.debug || false;
  }
}
function updateOptionsDisplay(options) {
  if (saverUrlDisplay) {
    const saverUrl = options.debug ? options.saver_url_debug : options.saver_url;
    saverUrlDisplay.textContent = saverUrl || "Not configured";
  }
  updateStatusDisplay(options);
}
function updateStatusDisplay(options) {
  if (!statusTextElement || !statusDotElement)
    return;
  const isActive = false;
  if (isActive) {
    statusTextElement.textContent = "Active";
    statusDotElement.classList.remove("inactive");
    statusDotElement.classList.add("active");
  } else {
    statusTextElement.textContent = "Inactive";
    statusDotElement.classList.remove("active");
    statusDotElement.classList.add("inactive");
  }
}
async function saveOptions() {
  try {
    const newOptions = {
      starts_in: startsInInput ? parseFloat(startsInInput.value) : 0.2,
      display_off_in: displayOffInput ? parseFloat(displayOffInput.value) : 1,
      run_on_battery: runOnBatteryInput ? runOnBatteryInput.checked : false,
      debug: debugInput ? debugInput.checked : false,
      saver_url: "",
      saver_url_debug: "",
      options_url: "",
      require_pass_in: 1
    };
    if (isNaN(newOptions.starts_in) || newOptions.starts_in < 0.1) {
      alert("Start After must be at least 0.1 minutes");
      return;
    }
    if (isNaN(newOptions.display_off_in) || newOptions.display_off_in < 0.5) {
      alert("Display Off must be at least 0.5 minutes");
      return;
    }
    await invoke("set_options", { options: newOptions });
    console.log("Options saved successfully");
    alert("Settings saved successfully!");
  } catch (error) {
    console.error("Failed to save options:", error);
    alert("Failed to save settings. Please try again.");
  }
}
async function resetOptions() {
  try {
    const defaultOptions = await invoke("factory_reset_options");
    console.log("Options reset to defaults:", defaultOptions);
    loadOptionsIntoForm(defaultOptions);
    await saveOptions();
    alert("Options reset to defaults");
  } catch (error) {
    console.error("Failed to reset options:", error);
    alert("Failed to reset options. Please try again.");
  }
}
async function previewScreensaver() {
  try {
    await invoke("preview_screensaver");
    console.log("Preview screensaver requested");
  } catch (error) {
    console.error("Failed to preview screensaver:", error);
    alert("Failed to preview screensaver. Please try again.");
  }
}
document.addEventListener("DOMContentLoaded", () => {
  init();
});
window.remoteOptions = {
  loadOptions,
  saveOptions,
  resetOptions
};
