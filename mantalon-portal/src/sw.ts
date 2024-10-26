/// <reference lib="WebWorker" />
/// <reference path="../node_modules/mantalon-client/mantalon_client.d.ts" />
export type {};
declare let self: ServiceWorkerGlobalScope;

import { config } from "process";
import { loadManifest, loadRessource, Manifest, RequestDirection, RewriteConfig, Substitution, SubstitutionConfig } from "./manifest";
import { Cookie, CookieJar } from "tough-cookie";
import { URLPattern } from "urlpattern-polyfill"; // TODO: When URLPatterns reaches baseline, remove this polyfill
type ProxiedFetchType = (arg1: any, arg2?: any) => Promise<Response>;

function orDefault(value: any, fallback: any) {
    return value !== undefined ? value : fallback;
}

var clientOrigins = new Map<string, string>();
var cookieJar = new CookieJar();

var initSuccess = false;
var initError = null;
var manifest: Manifest;
var globalProxiedFetch: ProxiedFetchType;

function addDefaultHeaders(headers: Headers, destination: RequestDestination, mode: RequestMode) {
    if (!headers.has("accept")) {
        let acceptValue: string;
        switch (destination) {
            case "": {
                acceptValue = "*/*";
                break;
            }
            case "audio": {
                acceptValue = "audio/*, */*;q=0.1";
                break;
            }
            case "audioworklet": {
                acceptValue = "*/*";
                break;
            }
            case "document": {
                acceptValue = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";
                break;
            }
            case "embed": {
                acceptValue = "*/*";
                break;
            }
            case "font": {
                acceptValue = "application/font-woff2;q=1.0,application/font-woff;q=0.9,*/*;q=0.8";
                break;
            }
            case "frame":
            case "iframe": {
                acceptValue = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
                break;
            }
            case "image": {
                acceptValue = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";
                break;
            }
            case "manifest": {
                acceptValue = "application/manifest+json,*/*;q=0.8";
                break;
            }
            case "object": {
                acceptValue = "*/*";
                break;
            }
            case "paintworklet": {
                acceptValue = "*/*";
                break;
            }
            case "report": {
                acceptValue = "*/*";
                break;
            }
            case "script": {
                acceptValue = "*/*";
                break;
            }
            case "sharedworker": {
                acceptValue = "*/*";
                break;
            }
            case "style": {
                acceptValue = "text/css,*/*;q=0.1";
                break;
            }
            case "track": {
                acceptValue = "*/*";
                break;
            }
            case "video": {
                acceptValue = "video/*, */*;q=0.1";
                break;
            }
            case "worker": {
                acceptValue = "*/*";
                break;
            }
            case "xslt": {
                acceptValue = "*/*";
                break;
            }
            default: {
                acceptValue = "*/*";
                break;
            }
        }
        headers.set("accept", acceptValue);
    }
    // TODO: Add accept-encoding header
    if (!headers.has("accept-language")) {
        headers.set("accept-language", "en-US,en;q=0.9");
    }
    // TODO: Add cache and pragma header defaults
    if (!headers.has("sec-ch-ua")) {
        headers.set("sec-ch-ua", '"Brave";v="129", "Not=A?Brand";v="8", "Chromium";v="129"');
    }
    if (!headers.has("sec-ch-ua-mobile")) {
        headers.set("sec-ch-ua-mobile", "?0");
    }
    if (!headers.has("sec-ch-ua-platform")) {
        headers.set("sec-ch-ua-platform", '"Linux"');
    }
    if (!headers.has("sec-fetch-dest")) {
        headers.set("sec-fetch-dest", destination);
    }
    if (!headers.has("sec-fetch-mode")) {
        if (mode === "navigate") {
            headers.set("sec-fetch-mode", "navigate");
        } else {
            headers.set("sec-fetch-mode", "no-cors");
        }
    }
    if (!headers.has("sec-fetch-site")) {
        if (mode === "navigate") {
            headers.set("sec-fetch-site", "none");
        } else {
            headers.set("sec-fetch-site", "same-origin");
        }
    }
    if (!headers.has("sec-fetch-user") && mode === "navigate") {
        headers.set("sec-fetch-user", "?1");
    }
    if (!headers.has("sec-gpc")) {
        headers.set("sec-gpc", "1");
    }
    if (!headers.has("upgrade-insecure-requests")) {
        headers.set("upgrade-insecure-requests", "1");
    }
    if (!headers.has("user-agent")) {
        headers.set("user-agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36");
    }
}

function applyHeaderChanges(headers: Headers, url: URL, isRequest: boolean) {
    let headersConfig = manifest.headers?.find((conf) => conf.matches.some((pattern) => pattern.test(url)));
    if (!headersConfig) {
        return;
    }

    for (let removeHeader of headersConfig.remove_headers || []) {
        let direction = orDefault(removeHeader.direction, RequestDirection.BOTH);
        if (
            direction === RequestDirection.BOTH
            || (isRequest && direction === RequestDirection.REQUEST)
            || (!isRequest && direction === RequestDirection.RESPONSE)
        ) {
            headers.delete(removeHeader.name);
        }
    }

    for (let renameHeader of headersConfig.rename_headers || []) {
        let direction = orDefault(renameHeader.direction, RequestDirection.BOTH);
        if (
            direction === RequestDirection.BOTH
            || (isRequest && direction === RequestDirection.REQUEST)
            || (!isRequest && direction === RequestDirection.RESPONSE)
        ) {
            let value = headers.get(renameHeader.name);
            if (value) {
                headers.delete(renameHeader.name);
                headers.set(renameHeader.name, value);
            }
        }
    }

    for (let addHeader of headersConfig.add_headers || []) {
        let direction = orDefault(addHeader.direction, RequestDirection.BOTH);
        if (
            direction === RequestDirection.BOTH
            || (isRequest && direction === RequestDirection.REQUEST)
            || (!isRequest && direction === RequestDirection.RESPONSE)
        ) {
            if (orDefault(addHeader.append, false)) {
                headers.append(addHeader.name, addHeader.value);
            } else {
                headers.set(addHeader.name, addHeader.value);
            }
        }
    }
}

async function applySubstitutions(response: Response, url: URL): Promise<ReadableStream<Uint8Array> | string | null> {
    if (!response.body) {
        return response.body;
    }

    let substitutionsConfig = manifest.substitutions?.find((conf) => conf.matches.some((pattern) => pattern.test(url)));
    let contentScriptsConfig = manifest.content_scripts?.find((conf) => conf.matches.some((pattern) => pattern.test(url)));

    if (!substitutionsConfig && !contentScriptsConfig) {
        return response.body;
    }

    if (!substitutionsConfig) {
        substitutionsConfig = new SubstitutionConfig({});
    }

    if (substitutionsConfig.substitutions.length == 0
        && orDefault(contentScriptsConfig?.js?.length, 0) == 0
        && orDefault(contentScriptsConfig?.css?.length, 0) == 0)
    {
        return response.body;
    }

    let bodyPromise = response.text(); // Start loading body while we load ressources

    if (contentScriptsConfig) {
        for (let css of contentScriptsConfig.css || []) {
            let contentResponse = await loadRessource(css);
            let content = await contentResponse?.text();
            if (!content) {
                console.error(`Couldn't inject css due to data being unavailable: ${css}`);
                continue;
            }
            substitutionsConfig.substitutions.push(new Substitution ({
                pattern: "<head>",
                replacement: `<style>${content}</style>`,
                insert: true,
                once: true,
                prevent_duplicates: true
            }));
        }
        for (let js of contentScriptsConfig.js || []) {
            let contentResponse = await loadRessource(js);
            let content = await contentResponse?.text();
            if (!content) {
                console.error(`Couldn't inject js due to data being unavailable: ${js}`);
                continue;
            }
            substitutionsConfig.substitutions.push(new Substitution ({
                pattern: "<head>",
                replacement: `<script>${content}</script>`,
                insert: true,
                once: true,
                prevent_duplicates: true
            }));
        }
    }

    let body = await bodyPromise;
    for (let substitution of substitutionsConfig.substitutions) {
        let pattern = substitution.pattern;
        let replacement = substitution.replacement;

        if (orDefault(substitution.prevent_duplicates, true)) {
            if (body.includes(replacement)) {
                continue;
            }
        }

        if (orDefault(substitution.insert, false)) {
            replacement = pattern + replacement;
        }

        if (orDefault(substitution.once, false)) {
            body = body.replace(pattern, replacement);
        } else {
            body = body.replaceAll(pattern, replacement);
        }
    }
    return body;
}

async function proxy(event: FetchEvent): Promise<Response> {
    // Get the actual URL of the request
    let url = new URL(event.request.url);
    if (url.origin === self.location.origin) {
        let clientOrigin = clientOrigins.get(event.clientId);
        if (!clientOrigin) {
            clientOrigin = manifest.targets[0];
            clientOrigins.set(event.clientId, clientOrigin);
        }
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
    let proxy_config = manifest.proxy_urls?.find((conf) => conf.matches.some((pattern) => pattern.test(url)));
    if (!proxy_config) {
        console.log("No proxy config found for ", url);
        return await fetch(event.request);
    }

    // Edit request headers
    let requestHeaders = new Headers(event.request.headers);
    addDefaultHeaders(requestHeaders, event.request.destination, event.request.mode);

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
    for (let [name, value] of responseHeaders.entries()) {
        if (name.startsWith("x-mantalon-set-cookie-")) {
            try {
                const resCookie = Cookie.parse(value);
                if (resCookie) {
                    cookieJar.setCookie(resCookie, url);
                }
            } catch (e) {
                console.error("Failed to parse set-cookie header", e);
            }
        }
    }

    // Apply header changes
    applyHeaderChanges(responseHeaders, url, false);

    // Apply substitutions
    let body = await applySubstitutions(initialResponse, url);
    
    let finalResponse = new Response(body, {
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
