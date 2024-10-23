/// <reference lib="WebWorker" />
/// <reference path="../node_modules/mantalon-client/mantalon_client.d.ts" />
export type {};
declare let self: ServiceWorkerGlobalScope;

import { loadManifest, Manifest } from "./manifest";
import { URLPattern } from "urlpattern-polyfill"; // TODO: When URLPatterns reaches baseline, remove this polyfill
type ProxiedFetchType = (arg1: any, arg2?: any) => Promise<Response>;

var initSuccess = false;
var initError = null;
var manifest: Manifest;
var globalProxiedFetch: ProxiedFetchType;

function proxy(event: FetchEvent) {
    let url = new URL(event.request.url);
    url.host = manifest.targets[0];
    if (!manifest.targets[0].includes(':')) {
        url.port = "443";
        url.protocol = "https:";
    }
    
    event.respondWith(globalProxiedFetch(url, {
        method: event.request.method,
        headers: event.request.headers,
        body: event.request.body,
    }));
}

self.addEventListener("fetch", (event: FetchEvent) => {
    if (initSuccess) {
        proxy(event);
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
