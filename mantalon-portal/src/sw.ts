/// <reference lib="WebWorker" />
/// <reference path="../node_modules/mantalon-client/mantalon_client.d.ts" />
export type {};
declare let self: ServiceWorkerGlobalScope;

import { config } from "process";
import { loadManifest, loadRessource, Manifest, RequestDirection, RewriteConfig, Substitution, SubstitutionConfig } from "./manifest";
import { Cookie, CookieJar } from "tough-cookie";
import { URLPattern } from "urlpattern-polyfill"; // TODO: When URLPatterns reaches baseline, remove this polyfill
import { applyJsProxy } from "./js-proxy";
import { addDefaultHeaders, applyHeaderChanges } from "./headers";
import { applySubstitutions } from "./substitutions";

type ProxiedFetchType = (arg1: any, arg2?: any) => Promise<Response>;

export function orDefault(value: any, fallback: any) {
    return value !== undefined ? value : fallback;
}

export var clientOrigins = new Map<string, string>();
export var cookieJar = new CookieJar();

var initSuccess = false;
var initError = null;
export var manifest: Manifest;
var globalProxiedFetch: ProxiedFetchType;

async function proxy(event: FetchEvent): Promise<Response> {
    // Get the actual URL of the request
    let url = new URL(event.request.url);
    let clientOrigin = clientOrigins.get(event.clientId);
    if (!clientOrigin) {
        clientOrigin = manifest.targets[0];
        clientOrigins.set(event.clientId, clientOrigin);
    }
    if (url.origin === self.location.origin) {
        let protocol = clientOrigin.substring(0, clientOrigin.indexOf(":"));
        let host = clientOrigin.substring(clientOrigin.indexOf(":") + 3);
        url.protocol = protocol;
        url.host = host;
        if (!host.includes(':')) {
            url.port = "443"; // TODO: refine port depending on protocol
        }
    }
    if (event.resultingClientId) {
        clientOrigins.set(event.resultingClientId, url.origin);
    }

    // Rewrite the URL if necessary
    if (manifest.rewrites) {
        let rewrite_config: RewriteConfig | undefined = undefined;
        let rewrite_match: URLPatternResult | undefined = undefined;
        for (let rewrite of manifest.rewrites) {
            for (let pattern of rewrite.matches) {
                let result = pattern.exec(url);
                if (result) {
                    rewrite_config = rewrite;
                    rewrite_match = result;
                    break;
                }
            }
            if (rewrite_match) {
                break;
            }
        }
        if (rewrite_config && rewrite_match) {
            let newUrl = new URL(rewrite_config.destination);
            let array: [{ [key: string]: string | undefined; }, string][] = [
                [rewrite_match.protocol.groups, "protocol"],
                [rewrite_match.username.groups, "username"],
                [rewrite_match.password.groups, "password"],
                [rewrite_match.hostname.groups, "hostname"],
                [rewrite_match.port.groups, "port"],
                [rewrite_match.pathname.groups, "pathname"],
                [rewrite_match.search.groups, "search"],
                [rewrite_match.hash.groups, "hash"],
            ]
            for (let [groups, target] of array) {
                for (let key in groups) {
                    newUrl[target] = newUrl[target].replace(`:${key}`, groups[key] || "");
                }
            }
            if (orDefault(rewrite_config.redirect, false)) {
                return Response.redirect(newUrl);
            } else {
                url = newUrl;
            }
        }
    }

    // Find the proxy config for the URL
    let proxy_config = manifest.proxy_urls?.find((conf) => conf.test(url, ""));
    if (!proxy_config) {
        console.log("No proxy config found for ", url);
        return await fetch(event.request);
    }

    // Edit request headers
    let requestHeaders = new Headers(event.request.headers);
    addDefaultHeaders(requestHeaders, event.request.destination, event.request.mode, clientOrigin);

    // Add cookies
    const matchingCookies = await cookieJar.getCookies(url);
    const cookieHeader = matchingCookies.map(cookie => cookie.cookieString()).join(';');
    if (cookieHeader.length > 0) {
        requestHeaders.set("cookie", cookieHeader);
    }

    // Apply header changes
    applyHeaderChanges(requestHeaders, url, true);

    // Clone the request if we might want to resend it
    let requestBody = event.request.body?.tee();

    let initialResponse = await globalProxiedFetch(url, {
        method: event.request.method,
        headers: requestHeaders,
        body: requestBody ? requestBody[0] : undefined
    });
    let bodyOverride: string | undefined;
    let contentType = initialResponse.headers.get("content-type") || "";

    // If the server asks for a single-chunk body, resend the request with the full body
    if (initialResponse.status == 411 && requestBody) {
        console.log("Resending request with full body");

        const reader = requestBody[1].getReader();
        let chunks: Uint8Array[] = [];
        while (true) {
            const {done, value} = await reader.read();
            if (done) break;
            chunks.push(value);
        }
        const requestFullBody = new Blob(chunks);
        
        initialResponse = await globalProxiedFetch(url, {
            method: event.request.method,
            headers: requestHeaders,
            body: requestFullBody,
        });
    }

    // Edit response headers
    let responseHeaders = new Headers(initialResponse.headers);
    if (orDefault(proxy_config.rewrite_location, true)) {
        let location = responseHeaders.get("location");
        if (location) {
            let newLocation = new URL(location, url);
            if (manifest.targets.includes(newLocation.origin)) {
                clientOrigins.set(event.clientId, newLocation.origin);
                newLocation.host = self.location.host;
                newLocation.protocol = self.location.protocol;
                newLocation.port = self.location.port;
            }
            responseHeaders.set("location", newLocation.toString());
        }
    }

    // Update cookies
    let cookieChanged = false;
    for (let [name, value] of responseHeaders.entries()) {
        if (name.startsWith("x-mantalon-set-cookie-")) {
            const resCookie = Cookie.parse(value);
            if (resCookie) {
                cookieJar.setCookie(resCookie, url);
                cookieChanged = true;
            }
        }
    }
    if (cookieChanged) {
        sendCookiesToClient(url);
    }

    // Apply header changes
    applyHeaderChanges(responseHeaders, url, false);

    // Apply js proxy
    let jsProxyResult = await applyJsProxy(initialResponse, url, contentType, event.clientId);
    if (jsProxyResult) {
        bodyOverride = jsProxyResult;
    }

    // Apply substitutions
    let substitutionResults = await applySubstitutions(bodyOverride || initialResponse, url, contentType);
    if (substitutionResults) {
        bodyOverride = substitutionResults;
    }
    
    let finalResponse = new Response(bodyOverride || initialResponse.body, {
        status: initialResponse.status,
        statusText: initialResponse.statusText,
        headers: responseHeaders,
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
    event.waitUntil(self.clients.claim());
});

self.addEventListener("message", event => {
    if (event.data.type === "mantalon-init-status") {
        if (initError) {
            event.source?.postMessage({ type: "mantalon-init-error", error: initError });
        } else if (initSuccess) {
            event.source?.postMessage({ type: "mantalon-init-success" });
        } else {
            event.source?.postMessage({ type: "mantalon-init-waiting" });
        }
    } else if (event.data.type === "mantalon-change-origin") {
        clientOrigins.set(event.data.clientId, event.data.origin);
        event.source?.postMessage({type: "mantalon-change-origin-success"});
    } else if (event.data.type === "mantalon-update-sw-cookie") {
        updateCookieFromClient(new URL(event.data.href), event.data.cookie);
    } else {
        console.error("Unknown message type", event.data.type);
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
function sendCookiesToClient(url: URL) {
    throw new Error("Function not implemented.");
}

function updateCookieFromClient(arg0: URL, cookie: any) {
    throw new Error("Function not implemented.");
}

