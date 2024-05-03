var initialized = false;
var currentHostnames = {}; // TODO: use authority

async function respond(request, clientId, replacesClientId) {
    // Directly fetch Mantalon resources
    let url = new URL(request.url);
    if (url.pathname.startsWith("/pkg/")
        || url.pathname.startsWith("/mantalon-connect/")
        || url.pathname === "/mantalon-connect"
        || url.pathname === "/sw.js") {
        return fetch(request);
    }

    // Handle artificial navigation requests
    if (url.href.startsWith(self.location.origin + "/mantalon/navigate?url=")) {
        let url_params = new URLSearchParams(url.search);
        let next_url_str = url_params.get("url");
        let next_url = new URL(next_url_str);
        currentHostnames[clientId] = next_url.hostname;
        next_url.protocol = self.location.protocol;
        next_url.hostname = self.location.hostname;
        next_url.port = self.location.port;
        return Response.redirect(next_url.href, 302);
    }

    // Wait for Mantalon to initialize
    while (!initialized) {
        await new Promise(resolve => setTimeout(resolve, 100));
    }

    // Proxy requests on selected domains
    if (self.proxiedDomains.includes(url.hostname)) {
        let resp = await proxiedFetch(request);
        if (replacesClientId) {
            let location = resp.headers.get("x-mantalon-location");
            let hostname = new URL(location).hostname;
            currentHostnames[replacesClientId] = hostname;
        }
        return resp;
    } else if (url.hostname == self.location.hostname) {
        if (!currentHostnames[clientId]) {
            currentHostnames[clientId] = self.proxiedDomains[0];
        }
        url.protocol = "https"; // TODO support http proxied sites
        url.hostname = currentHostnames[clientId];
        url.port = ""; // TODO support proxied sites with port
        let resp = await proxiedFetch(request, url.href);
        if (replacesClientId) {
            let location = resp.headers.get("x-mantalon-location");
            let hostname = new URL(location).hostname;
            currentHostnames[replacesClientId] = hostname;
        }
        return resp;
    } else {
        return fetch(request);
    }
}

// Listen for fetch events
self.addEventListener("fetch", (event) => {
    event.respondWith(respond(event.request, event.clientId, event.replacesClientId)) // We need an inner function to be able to respond asynchronously
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
