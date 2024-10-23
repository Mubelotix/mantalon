/// <reference lib="WebWorker" />
/// <reference path="../node_modules/mantalon-client/mantalon_client.d.ts" />
export type {};
declare let self: ServiceWorkerGlobalScope;

import { config } from "process";
import { loadManifest, Manifest } from "./manifest";
import { URLPattern } from "urlpattern-polyfill"; // TODO: When URLPatterns reaches baseline, remove this polyfill
type ProxiedFetchType = (arg1: any, arg2?: any) => Promise<Response>;

function orDefault(value: any, fallback: any) {
    return value !== undefined ? value : fallback;
}

var initSuccess = false;
var initError = null;
var manifest: Manifest;
var globalProxiedFetch: ProxiedFetchType;

function setCurrentHost(host: string) {

}

async function proxy(event: FetchEvent): Promise<Response> {
    let url = new URL(event.request.url);
    url.host = manifest.targets[0];
    if (!manifest.targets[0].includes(':')) {
        url.port = "443";
        url.protocol = "https:";
    }

    let proxy_config = manifest.proxy_urls?.find((conf) => conf.matches.some((pattern) => pattern.test(url)));
    if (!proxy_config) {
        return await fetch(event.request);
    }

    let initialResponse = await globalProxiedFetch(url, {
        method: event.request.method,
        headers: event.request.headers,
        body: event.request.body,
    });

    let headers = new Headers(initialResponse.headers);

    if (orDefault(proxy_config.rewrite_location, true)) {
        console.log("Rewriting location headers");
        let location = headers.get("location");
        console.log("Location: ", location);
        if (location) {
            let newLocation = new URL(location, url);
            if (manifest.targets.includes(newLocation.host)) {
                setCurrentHost(newLocation.host);
                newLocation.host = self.location.host;
                newLocation.protocol = self.location.protocol;
            }
            headers.set("location", newLocation.toString());

            console.log("New Location: ", newLocation.toString());
        } else {
            console.log("No location header found");
        }
    }
    
    let finalResponse = new Response(initialResponse.body, {
        status: initialResponse.status,
        statusText: initialResponse.statusText,
        headers: headers,
    });

    return finalResponse;
}

self.addEventListener("fetch", (event: FetchEvent) => {
    if (initSuccess) {
        event.respondWith(proxy(event));
    } else {
        event.respondWith(new Response("Service Worker not yet initialized"));
    }
});

self.addEventListener("install", (event) => {
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
    self.clients.claim();
});

self.addEventListener('message', event => {
    if (event.data.type === 'mantalon-init-status') {
        if (initError) {
            event.source?.postMessage({ type: "mantalon-init-error", error: initError });
        } else if (initSuccess) {
            event.source?.postMessage({ type: "mantalon-init-waiting" });
        } else {
            event.source?.postMessage({ type: "mantalon-init-success" });
        }
    }
});

try {
    importScripts("/mantalon/mantalon_client.js");

    let loadingManifest = loadManifest();

    const { init, proxiedFetch } = wasm_bindgen;
    async function run() {
        await wasm_bindgen("/mantalon/mantalon_client_bg.wasm");
        manifest = await loadingManifest;
        await init(manifest.server_endpoint);
        initSuccess = true;
        globalProxiedFetch = proxiedFetch;1
        console.log("Successfully initialized Mantalon. Proxying ");
    }

    run().catch((e) => {
        initError = e;
        console.error("Failed to initialize Mantalon", e);
    })
} catch (e) {
    initError = e;
    console.error("Failed to load Mantalon", e);
}
