const CACHE = "opendesk-shell-v1";
const SHELL = ["/", "/index.html", "/manifest.webmanifest", "/opendesk.svg"];

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(SHELL)));
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((key) => key !== CACHE).map((key) => caches.delete(key))),
      ),
  );
  self.clients.claim();
});

self.addEventListener("fetch", (event) => {
  if (event.request.method !== "GET") return;
  const url = new URL(event.request.url);
  if (url.pathname.startsWith("/api/") || url.pathname === "/ws") return;
  if (event.request.mode === "navigate") {
    event.respondWith(
      fetch(event.request).catch(() => caches.match("/index.html").then((hit) => hit || Response.error())),
    );
    return;
  }
  event.respondWith(
    caches.match(event.request).then((hit) => hit || fetch(event.request)),
  );
});

self.addEventListener("push", (event) => {
  let payload = {};
  try {
    payload = event.data ? event.data.json() : {};
  } catch {
    payload = {};
  }
  const title = payload.title || "OpenDesk";
  const url = payload.url || "/";
  event.waitUntil(
    self.registration.showNotification(title, {
      body: payload.body || "",
      icon: "/opendesk.svg",
      badge: "/opendesk.svg",
      tag: payload.tag || undefined,
      renotify: Boolean(payload.tag),
      data: { url },
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const target = event.notification.data && event.notification.data.url
    ? event.notification.data.url
    : "/";
  event.waitUntil(
    self.clients
      .matchAll({ type: "window", includeUncontrolled: true })
      .then((windows) => {
        for (const client of windows) {
          if ("focus" in client) {
            client.navigate(target).catch(() => undefined);
            return client.focus();
          }
        }
        return self.clients.openWindow(target);
      }),
  );
});
