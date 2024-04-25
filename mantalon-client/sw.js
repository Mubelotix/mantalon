import init, { proxiedFetch } from '/mantalon/pkg/mantalon_client.js';

async function run() {
    await init();
    self.proxiedFetch = proxiedFetch;
    console.log("Initialized");
}
run();

self.addEventListener("fetch", (event) => {
    if (event.request.url.pathname.startsWith("/mantalon/")
        || event.request.url.pathname.startsWith("/mantalon-connect/")
        || event.request.url.pathname === "/mantalon-connect"
        || event.request.url.pathname === "/sw.js") {
        event.respondWith(fetch(event.request));
        return;
    }
    event.respondWith(proxiedFetch(event.request));
});
