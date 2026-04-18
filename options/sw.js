// Service worker for the reference options page.
// Strategy: stale-while-revalidate for all assets — serves from cache instantly,
// updates in the background so the next load is fresh.

const CACHE = 'liminal-options-v1';

// Assets to pre-cache on install
const PRECACHE = [
  './',
  './index.html',
  './main.js',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(PRECACHE)).then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  // Delete old caches
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))),
    ).then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  // Only handle GET requests for same-origin assets
  if (event.request.method !== 'GET') return;

  event.respondWith(
    caches.open(CACHE).then(async (cache) => {
      const cached = await cache.match(event.request);

      const fetchPromise = fetch(event.request)
        .then((response) => {
          if (response.ok) cache.put(event.request, response.clone());
          return response;
        })
        .catch(() => null);

      // Return cached immediately, update in background
      return cached ?? await fetchPromise ?? new Response('Offline', { status: 503 });
    }),
  );
});
