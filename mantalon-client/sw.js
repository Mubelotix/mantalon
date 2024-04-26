// Create a wrapper function to wait until the proxiedFetch function is loaded
async function proxiedFetchWrapper(request) {
    while (!self.proxiedFetch) {
        await new Promise(resolve => setTimeout(resolve, 100));
    }
    return self.proxiedFetch(request);
}

// Listen for fetch events
self.addEventListener("fetch", (event) => {
    let request = event.request;
    let url = new URL(request.url);
    if (url.pathname.startsWith("/pkg/")
        || url.pathname.startsWith("/mantalon-connect/")
        || url.pathname === "/mantalon-connect"
        || url.pathname === "/sw.js") {
        event.respondWith(fetch(event.request));
        return;
    }
    if ((url.hostname === "127.0.0.1" || url.hostname === "en.wikipedia.org") && url.pathname != "/") {
        console.log("Proxying", url.href);
        event.respondWith(proxiedFetchWrapper(event.request));
        return;
    } else {
        console.log("ignoring", url.href);
    }
});

// Load Mantalon library
import init, { proxiedFetch } from '/pkg/mantalon_client.js';
async function run() {
    await init();
    self.proxiedFetch = proxiedFetch;
    console.log("Initialized");
}
run();
