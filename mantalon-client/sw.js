import init, { proxiedFetch } from '/pkg/mantalon_client.js';

async function run() {
    await init();
    self.proxiedFetch = proxiedFetch;
    console.log("Initialized");
}
run();

self.addEventListener("fetch", (event) => {
    event.respondWith(fetch(event.request));
});
