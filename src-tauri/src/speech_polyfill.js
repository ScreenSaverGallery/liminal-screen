// Web Speech API (speechSynthesis) polyfill for webviews without a native
// implementation — WebKitGTK on Linux does not ship one. Injected at
// document-start into saver and preview windows (see speech.rs). Steps aside
// when the native API exists (macOS WKWebView, Windows WebView2) or when the
// Tauri IPC bridge is unavailable (e.g. remote page without IPC injection).
(function () {
  if ('speechSynthesis' in window) return;
  var internals = window.__TAURI_INTERNALS__;
  if (!internals || typeof internals.invoke !== 'function') {
    if (window.console && window.console.warn) {
      window.console.warn(
        '[liminal-screen] speechSynthesis unavailable: no native API and no IPC bridge'
      );
    }
    return;
  }
  var invoke = internals.invoke.bind(internals);

  function SpeechSynthesisUtterance(text) {
    this.text = text === undefined || text === null ? '' : String(text);
    this.lang = '';
    this.voice = null;
    this.volume = 1;
    this.rate = 1;
    this.pitch = 1;
    this.onstart = null;
    this.onend = null;
    this.onerror = null;
    this.onpause = null;
    this.onresume = null;
    this.onmark = null;
    this.onboundary = null;
  }

  // cancel() bumps the generation; utterances queued under an older
  // generation drain without speaking (they still fire `end`, like the
  // native API does for cancelled utterances).
  var generation = 0;
  var queue = Promise.resolve();
  var activeCount = 0;

  function fire(utterance, name, extra) {
    var handler = utterance['on' + name];
    if (typeof handler !== 'function') return;
    var event = { type: name, utterance: utterance, charIndex: 0, elapsedTime: 0, name: '' };
    if (extra) for (var k in extra) event[k] = extra[k];
    try {
      handler.call(utterance, event);
    } catch (e) {
      // listener errors must not break the utterance queue
      if (window.console && window.console.error) window.console.error(e);
    }
  }

  var synthesis = {
    get speaking() {
      return activeCount > 0;
    },
    get pending() {
      return activeCount > 1;
    },
    paused: false,
    onvoiceschanged: null,
    getVoices: function () {
      return [];
    },
    speak: function (utterance) {
      var gen = generation;
      activeCount++;
      queue = queue.then(function () {
        if (gen !== generation) {
          activeCount--;
          fire(utterance, 'end');
          return;
        }
        fire(utterance, 'start');
        // speak_text resolves when the utterance has finished (spd-say -w),
        // so `end` below is truthful, not an estimate.
        return invoke('speak_text', {
          text: utterance.text,
          rate: utterance.rate,
          pitch: utterance.pitch,
          volume: utterance.volume,
          lang: utterance.lang || null,
        }).then(
          function () {
            activeCount--;
            fire(utterance, 'end');
          },
          function (error) {
            activeCount--;
            fire(utterance, 'error', { error: String(error) });
          }
        );
      });
    },
    cancel: function () {
      generation++;
      invoke('cancel_speech').catch(function () {});
    },
    // spd-say has no pause control; no-ops keep callers from crashing
    pause: function () {},
    resume: function () {},
    addEventListener: function () {},
    removeEventListener: function () {},
    dispatchEvent: function () {
      return false;
    },
  };

  window.SpeechSynthesisUtterance = SpeechSynthesisUtterance;
  window.speechSynthesis = synthesis;
})();
