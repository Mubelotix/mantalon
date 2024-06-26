var initialized = false;
var currentHostnames = {}; // TODO: use authority

async function respond(request, clientId, replacesClientId) {
    // Directly fetch Mantalon resources
    let url = new URL(request.url);
    let url_params = new URLSearchParams(url.search);
    let mantalonProtocol = url_params.get("mantalon-protocol");
    let mantalonHost = url_params.get("mantalon-host");
    let mantalonNavigate = url_params.get("mantalon-navigate");
    url_params.delete("mantalon-protocol");
    url_params.delete("mantalon-host");
    url_params.delete("mantalon-navigate");
    url.search = url_params.toString();
    if (url.pathname.startsWith("/pkg/")
        || url.pathname.startsWith("/mantalon-connect/")
        || url.pathname === "/mantalon-connect"
        || url.pathname === "/sw.js") {
        return fetch(request);
    }

    // Handle artificial navigation requests
    if (mantalonNavigate === "true") {
        if (mantalonHost) {
            currentHostnames[clientId] = mantalonHost;
        } else {
            console.error("No mantalon-host provided for navigation request");
        }
    }

    // Wait for Mantalon to initialize
    while (!initialized) {
        await new Promise(resolve => setTimeout(resolve, 100));
    }

    // Handle override cookie requests
    if (url.pathname === "/mantalon-override-cookie") {
        let cookie_name = url_params.get("name");
        let cookie_value = url_params.get("value");
        overrideCookie(cookie_name, cookie_value);
        return new Response("Cookie overridden", { status: 200 });
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
    } else if (url.host == self.location.host) {
        if (!currentHostnames[clientId]) {
            currentHostnames[clientId] = self.proxiedDomains[0];
        }
        if (mantalonProtocol && mantalonHost) {
            url.protocol = mantalonProtocol;
            url.hostname = mantalonHost.split(":")[0];
            url.port = mantalonHost.split(":")[1] || "";
        } else {
            url.protocol = "https"; // TODO support http proxied sites
            url.hostname = currentHostnames[clientId];
            url.port = ""; // TODO support proxied sites with port
        }
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

self.addEventListener("install", (event) => {
  self.skipWaiting();
});

var initError = null;
try {
    importScripts("/pkg/mantalon_client.js?version=LIB_VERSION");

    const { init, proxiedFetch, getProxiedDomains, overrideCookie } = wasm_bindgen;
    async function run() {
        await wasm_bindgen("/pkg/mantalon_client_bg.wasm?version=LIB_VERSION");
        await init("/pkg/config/manifest.json?version=MANIFEST_VERSION");
        self.proxiedFetch = proxiedFetch;
        self.proxiedDomains = getProxiedDomains();
        self.overrideCookie = overrideCookie;
        initialized = true;
        console.log("Successfully initialized Mantalon. Proxying ", self.proxiedDomains);
    }

    run().catch((e) => {
        initError = e;
        console.error("Failed to initialize Mantalon", e);
    })
} catch (e) {
    initError = e;
    console.error("Failed to load Mantalon", e);
}

self.addEventListener('message', event => {
    if (event.data.type === 'mantalon-init-status') {
        if (initError) {
            event.source.postMessage({ type: "mantalon-init-error", error: initError });
        } else {
            event.source.postMessage({ type: "mantalon-init-success" });
        }
    }
});
