// Sample screensaver content that emits activity detection events
// This would be part of the content loaded in the saver windows

(function() {
  // Emit activity detection event when user interacts
  function emitActivity() {
    // Send event to main application
    if (window.__TAURI__) {
      window.__TAURI__.event.emit('saver-activity-detected');
    }
    
    // Also try to emit through any available window reference
    if (window.parent && window.parent.window) {
      window.parent.window.dispatchEvent(new CustomEvent('saver-activity-detected'));
    }
  }

  // Add event listeners for various user interactions
  document.addEventListener('mousemove', emitActivity, { passive: true });
  document.addEventListener('keydown', emitActivity, { passive: true });
  document.addEventListener('mousedown', emitActivity, { passive: true });
  document.addEventListener('touchstart', emitActivity, { passive: true });
  document.addEventListener('touchmove', emitActivity, { passive: true });
  document.addEventListener('wheel', emitActivity, { passive: true });
  
  // For mobile/touch devices
  window.addEventListener('deviceorientation', emitActivity, { passive: true });
  window.addEventListener('devicemotion', emitActivity, { passive: true });
  
  console.log('Activity detection initialized for screensaver window');
})();