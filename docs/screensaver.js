(function () {
  'use strict';

  // =========================================================================
  // Classic starfield — a la Windows 95.
  // Stars fly outward from the center, growing larger and faster as they
  // approach the camera. That's it. No input, no frills, just the void.
  // =========================================================================

  const canvas = document.getElementById('stage');
  const ctx = canvas.getContext('2d', { alpha: false });

  const STAR_COUNT = 300;
  const SPEED = 4;        // forward speed per frame at 60fps
  const DEPTH = 256;      // z range: 1 (at camera) .. DEPTH (far)

  let width = 0, height = 0, cx = 0, cy = 0;
  let stars = [];

  function rand(min, max) { return min + Math.random() * (max - min); }

  function spawnStar(initial) {
    return {
      x: rand(-width / 2, width / 2),
      y: rand(-height / 2, height / 2),
      z: initial ? rand(1, DEPTH) : DEPTH,
    };
  }

  function createStars() {
    stars = [];
    for (let i = 0; i < STAR_COUNT; i++) stars.push(spawnStar(true));
  }

  function resize() {
    width = window.innerWidth;
    height = window.innerHeight;
    canvas.width = width;
    canvas.height = height;
    cx = width / 2;
    cy = height / 2;
  }

  let lastFrame = 0;

  function loop(timeMs) {
    const dt = Math.min(64, timeMs - lastFrame) / 16.67; // normalize to 60fps
    lastFrame = timeMs;

    // Clear to deep black.
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, width, height);

    const dz = SPEED * dt;
    for (let i = 0; i < stars.length; i++) {
      const s = stars[i];
      s.z -= dz;
      if (s.z <= 1) {
        s.x = rand(-width / 2, width / 2);
        s.y = rand(-height / 2, height / 2);
        s.z = DEPTH;
      }

      const k = 128 / Math.max(s.z, 1);   // perspective scale
      const px = cx + s.x * k;
      const py = cy + s.y * k;

      if (px < 0 || px > width || py < 0 || py > height) continue;

      const size = Math.max(0.5, (1 - s.z / DEPTH) * 2.5);

      ctx.fillStyle = '#fff';
      ctx.fillRect(px - size / 2, py - size / 2, size, size);
    }

    requestAnimationFrame(loop);
  }

  function init() {
    resize();
    createStars();
    lastFrame = performance.now();
    requestAnimationFrame(loop);
  }

  window.addEventListener('resize', () => {
    resize();
    createStars();
  });

  // Reduce motion: a single static field of faint stars.
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    resize();
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, width, height);
    ctx.fillStyle = '#fff';
    for (let i = 0; i < 150; i++) {
      ctx.fillRect(Math.random() * width, Math.random() * height, 1, 1);
    }
    return;
  }

  init();
})();