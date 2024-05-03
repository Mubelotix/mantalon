var initialized = false;

async function respond(request) {
    // Directly fetch Mantalon resources
    let url = new URL(request.url);
    if (url.pathname.startsWith("/pkg/")
        || url.pathname.startsWith("/mantalon-connect/")
        || url.pathname === "/mantalon-connect"
        || url.pathname === "/sw.js") {
        return fetch(request);
    }

    // Wait for Mantalon to initialize
    while (!initialized) {
        await new Promise(resolve => setTimeout(resolve, 100));
    }

    // Proxy requests on selected domains
    if (self.proxiedDomains.includes(url.hostname)) {
        return proxiedFetch(request);
    } else if (url.hostname == self.location.hostname) {
        url.protocol = "https"; // TODO support http proxied sites
        url.hostname = self.proxiedDomains[0];
        url.port = ""; // TODO support proxied sites with port
        return proxiedFetch(request, url.href);
    } else {
        return fetch(request);
    }
}

// Listen for fetch events
self.addEventListener("fetch", (event) => {
    event.respondWith(respond(event.request)) // We need an inner function to be able to respond asynchronously
});

// Load Mantalon library
import initWasm, { init, proxiedFetch, getProxiedDomains } from '/pkg/mantalon_client.js';
async function run() {
    await initWasm();
    await init();
    self.proxiedFetch = proxiedFetch;
    self.proxiedDomains = getProxiedDomains();
    initialized = true;
    console.log("Successfully initialized Mantalon. Proxying ", self.proxiedDomains);
}
run();
