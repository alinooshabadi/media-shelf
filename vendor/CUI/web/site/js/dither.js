// A grayscale photo drawn as an ordered (8x8 Bayer) dither in a small natural
// palette: blues for the sky and snow, olive and sand for the valley, pale
// blue for the water. Light theme: deep inks where the photo is dark. Dark
// theme: light inks where it is light. So the same picture reads as a print in
// both. Each dot is a whole number of device pixels, so it stays crisp at any
// pixel ratio.

const BAYER = (() => {
  let m = [[0]];
  while (m.length < 8) {
    const n = m.length, next = [];
    for (let y = 0; y < n * 2; y++) {
      next.push([]);
      for (let x = 0; x < n * 2; x++) next[y].push(4 * m[y % n][x % n] + [0, 2, 3, 1][(y >= n) * 2 + (x >= n)]);
    }
    m = next;
  }
  return Float32Array.from(m.flat(), (v) => (v + 0.5) / 64);
})();

const sources = new Map();
function loadImage(src) {
  if (!sources.has(src)) {
    sources.set(src, new Promise((resolve, reject) => {
      const img = new Image();
      img.decoding = 'async';
      img.onload = () => resolve(img);
      img.onerror = reject;
      img.src = src;
    }));
  }
  return sources.get(src);
}

const ease = (t) => 1 - Math.pow(1 - t, 3);
const mix = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
const smooth = (a, b, x) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };

// sky: storm blue to cloud; ground: olive to sand, with the brightest (water) back to pale blue
const PALETTE = {
  dark: { sky: [74, 146, 222], cloud: [178, 206, 232], olive: [138, 168, 78], sand: [222, 196, 122] },
  light: { sky: [28, 96, 172], cloud: [70, 118, 160], olive: [78, 108, 30], sand: [150, 112, 30] },
};
function tint(pal, lum, groundW) {
  const sky = mix(pal.sky, pal.cloud, lum);
  const ground = lum > 0.72 ? mix(pal.sand, pal.cloud, smooth(0.72, 0.95, lum)) : mix(pal.olive, pal.sand, lum / 0.72);
  return mix(sky, ground, groundW);
}

/* opts: src, cell (CSS px per dot), focusY (0 top .. 1 bottom: which part of
 * the photo stays in view when it is cropped), horizon (0..1 of the photo
 * where the sky gives way to the ground), gamma (higher = fewer dots),
 * reveal (true: the picture fades in on first draw; 'view': the same, once
 * the art scrolls into view), loop (a soft band of light sweeps across
 * the picture and repeats, like cloud shadows over the valley; it pauses
 * off screen and never runs with reduced motion), reduced () => bool. */
export function ditherArt(opts) {
  const o = { cell: 2, focusY: 0.5, horizon: 0.53, gamma: 1, reveal: true, loop: false, reduced: () => false, ...opts };
  const PERIOD = 14;   // seconds for one sweep of the light
  const el = document.createElement('div');
  el.className = 'dither';
  el.setAttribute('aria-hidden', 'true');
  const canvas = document.createElement('canvas');
  el.append(canvas);

  let img = null, lum = null, rowY = null, gw = 0, gh = 0, k = 1, raf = 0, revealed = !o.reveal;
  let small = null, sctx = null, data = null, visible = o.reveal !== 'view';
  const ctx = canvas.getContext('2d');

  function measure() {
    const r = el.getBoundingClientRect();
    if (!img || r.width < 1 || r.height < 1) return false;
    const dpr = window.devicePixelRatio || 1;
    k = Math.max(1, Math.round(o.cell * dpr));
    gw = Math.ceil(r.width * dpr / k);
    gh = Math.ceil(r.height * dpr / k);
    // cover: crop the photo to the box's shape, keeping focusY in view
    const ia = img.naturalWidth / img.naturalHeight, ba = gw / gh;
    let sw = img.naturalWidth, sh = img.naturalHeight, sx = 0, sy = 0;
    if (ia > ba) { sw = sh * ba; sx = (img.naturalWidth - sw) / 2; }
    else { sh = sw / ba; sy = (img.naturalHeight - sh) * o.focusY; }
    small = document.createElement('canvas');
    small.width = gw; small.height = gh;
    sctx = small.getContext('2d', { willReadFrequently: true });
    sctx.imageSmoothingQuality = 'high';
    sctx.drawImage(img, sx, sy, sw, sh, 0, 0, gw, gh);
    const px = sctx.getImageData(0, 0, gw, gh).data;
    rowY = Float32Array.from({ length: gh }, (_, y) => (sy + (y + 0.5) / gh * sh) / img.naturalHeight);
    lum = new Float32Array(gw * gh);
    for (let i = 0; i < lum.length; i++) lum[i] = px[i * 4] / 255;
    data = sctx.createImageData(gw, gh);
    canvas.width = gw * k; canvas.height = gh * k;
    canvas.style.width = `${gw * k / dpr}px`;
    canvas.style.height = `${gh * k / dpr}px`;
    return true;
  }

  // progress 0..1 develops the dots from the top down; time (seconds) moves
  // the band of light when looping
  function paint(progress, time = loopTime()) {
    if (!lum) return;
    const dark = document.documentElement.dataset.theme === 'dark';
    const sweep = time === null ? null : (time / PERIOD) * Math.PI * 2;
    const pal = dark ? PALETTE.dark : PALETTE.light;
    const d = data.data;
    for (let y = 0; y < gh; y++) {
      const t = progress >= 1 ? 1 : ease(Math.min(1, Math.max(0, progress * 1.5 - (y / gh) * 0.5)));
      const groundW = smooth(o.horizon - 0.05, o.horizon + 0.05, rowY[y]);
      for (let x = 0; x < gw; x++) {
        const i = y * gw + x, j = i * 4;
        let v = dark ? lum[i] : 1 - lum[i];
        v = Math.pow(v, o.gamma) * t;
        if (sweep !== null) {
          // a diagonal band: dark theme gains light dots, light theme loses ink
          const w = 0.5 + 0.5 * Math.cos((x / gw * 0.9 + y / gh * 0.35) * Math.PI * 2 - sweep);
          const band = w * w * w * w * w * w;
          v *= dark ? 1 + 0.55 * band : 1 - 0.45 * band;
        }
        if (v > BAYER[(y & 7) * 8 + (x & 7)]) {
          const c = tint(pal, lum[i], groundW);
          d[j] = c[0]; d[j + 1] = c[1]; d[j + 2] = c[2]; d[j + 3] = 255;
        } else d[j + 3] = 0;
      }
    }
    sctx.putImageData(data, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(small, 0, 0, gw * k, gh * k);
  }

  // Paint once, then fade the canvas in with CSS: the compositor runs the fade,
  // so it stays smooth while the page is busy starting its live previews.
  // The light sweep only starts after the fade, so one painter at a time.
  let loopReady = false, loopDelay = 0;
  function draw() {
    cancelAnimationFrame(raf);
    if (!measure()) return;
    if (!visible) { paint(1); return; }   // 'view': drawn but hidden until it scrolls in
    paint(1);
    if (!revealed) {
      revealed = true;
      if (!o.reveal || o.reduced()) el.classList.add('shown', 'instant');
      else raf = requestAnimationFrame(() => el.classList.add('shown'));
      clearTimeout(loopDelay);
      loopDelay = setTimeout(() => { loopReady = true; loopStart(); }, o.reduced() ? 0 : 1600);
    } else loopStart();
  }

  let loopRaf = 0, loopLast = 0, inView = false;
  const t0 = performance.now();
  function loopTime() { return o.loop && !o.reduced() && loopReady ? (performance.now() - t0) / 1000 : null; }
  function loopTick(now) {
    loopRaf = 0;
    if (!el.isConnected || !o.loop || o.reduced() || !loopReady || !inView || document.hidden) return;
    if (now - loopLast >= 50 && lum) { loopLast = now; paint(1); }
    loopRaf = requestAnimationFrame(loopTick);
  }
  function loopStart() { if (o.loop && loopReady && !loopRaf) loopRaf = requestAnimationFrame(loopTick); }
  const vio = new IntersectionObserver((es) => { inView = es.some((e) => e.isIntersecting); if (inView) loopStart(); }, { threshold: 0 });
  const onVis = () => { if (!document.hidden) loopStart(); };
  document.addEventListener('visibilitychange', onVis);

  // size changes while fonts and layout settle collapse into one redraw
  let lastW = 0, lastH = 0, pending = 0, settle = 0, seen = false;
  const ro = new ResizeObserver(([e]) => {
    const { width, height } = e.contentRect;
    if (el.isConnected) seen = true;
    else { if (seen) stop(); return; }
    if (Math.abs(width - lastW) < 1 && Math.abs(height - lastH) < 1) return;
    lastW = width; lastH = height;
    cancelAnimationFrame(pending); clearTimeout(settle);
    if (!lum) pending = requestAnimationFrame(draw);
    else settle = setTimeout(draw, 150);
  });
  let themeRaf = 0;
  const mo = new MutationObserver(() => {
    if (!el.isConnected) { if (seen) stop(); return; }
    if (lum && !themeRaf) themeRaf = requestAnimationFrame(() => { themeRaf = 0; paint(1); });
  });
  const io = new IntersectionObserver((es) => {
    if (!es.some((e) => e.isIntersecting)) return;
    io.disconnect();
    if (!visible) { visible = true; if (lum) draw(); }
  }, { threshold: 0.35 });
  function stop() {
    ro.disconnect(); mo.disconnect(); io.disconnect(); vio.disconnect(); document.removeEventListener('visibilitychange', onVis);
    cancelAnimationFrame(raf); cancelAnimationFrame(pending); cancelAnimationFrame(loopRaf); cancelAnimationFrame(themeRaf);
    clearTimeout(settle); clearTimeout(loopDelay);
  }

  loadImage(o.src).then((i) => {
    img = i;
    ro.observe(el);
    if (!visible) io.observe(el);
    if (o.loop) vio.observe(el);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'style'] });
  }).catch(() => {});
  return el;
}
