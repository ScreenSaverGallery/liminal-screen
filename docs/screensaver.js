
(function () {
  const canvas = document.getElementById('stage');
  const ctx = canvas.getContext('2d');

  let width = 0;
  let height = 0;
  let particles = [];
  let hue = 220;

  const PARTICLE_COUNT = 90;
  const SPEED = 0.4;

  function resize() {
    width = window.innerWidth;
    height = window.innerHeight;
    canvas.width = width;
    canvas.height = height;
  }

  function createParticles() {
    particles = [];
    for (let i = 0; i < PARTICLE_COUNT; i++) {
      particles.push({
        x: Math.random() * width,
        y: Math.random() * height,
        vx: (Math.random() - 0.5) * SPEED,
        vy: (Math.random() - 0.5) * SPEED,
        radius: Math.random() * 2 + 1,
      });
    }
  }

  function draw() {
    // Fade trail
    ctx.fillStyle = 'rgba(5, 5, 8, 0.22)';
    ctx.fillRect(0, 0, width, height);

    // Slowly shift color
    hue = (hue + 0.15) % 360;
    const color = `hsla(${hue}, 70%, 60%, 0.85)`;

    for (const p of particles) {
      p.x += p.vx;
      p.y += p.vy;

      if (p.x < 0 || p.x > width) p.vx *= -1;
      if (p.y < 0 || p.y > height) p.vy *= -1;

      ctx.beginPath();
      ctx.arc(p.x, p.y, p.radius, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
    }

    // Connect nearby particles with faint lines
    ctx.strokeStyle = `hsla(${hue}, 70%, 60%, 0.08)`;
    ctx.lineWidth = 0.5;
    for (let i = 0; i < particles.length; i++) {
      for (let j = i + 1; j < particles.length; j++) {
        const a = particles[i];
        const b = particles[j];
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const dist = Math.hypot(dx, dy);
        if (dist < 120) {
          ctx.beginPath();
          ctx.moveTo(a.x, a.y);
          ctx.lineTo(b.x, b.y);
          ctx.stroke();
        }
      }
    }

    requestAnimationFrame(draw);
  }

  function init() {
    resize();
    createParticles();
    draw();
  }

  window.addEventListener('resize', () => {
    resize();
    createParticles();
  });

  // Reduce motion preference support
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    ctx.fillStyle = '#050508';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    return;
  }

  init();
})();
