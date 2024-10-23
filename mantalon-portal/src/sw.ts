/// <reference lib="WebWorker" />
/// <reference path="../node_modules/mantalon-client/mantalon_client.d.ts" />
export type {};
declare let self: ServiceWorkerGlobalScope;

import { } from "./manifest";

var initSuccess = false;
var initError = null;

self.addEventListener("fetch", (event: FetchEvent) => {
  event.respondWith(new Response("Hello, world!"));
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
    importScripts("/node_modules/mantalon-client/mantalon_client.js");

    const { init, proxiedFetch } = wasm_bindgen;
    async function run() {
        await wasm_bindgen("/node_modules/mantalon-client/mantalon_client_bg.wasm");
        await init("http://localhost:1234/mantalon-connect");
        initSuccess = true;
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
