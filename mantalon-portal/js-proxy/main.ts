// TODO: Message passing
// TODO: Parent window
// TODO: window.cookieStore

import { makeProxiedDocument } from "./document";
import { fromFakeUrl, intoFakeUrl, makeProxiedLocation } from "./location";
import { makeProxiedWindow } from "./window";

const currentOrigin = "init-origin"; // Value is added automatically when the script gets injected
const clientId = "init-clientId"; // Value is added automatically when the script gets injected
const targetOrigins = new Set(["init-targetOrigins"]); // Value is added automatically when the script gets injected
var cookies = "init-cookies"; // Value is added automatically when the script gets injected
const currentHost = currentOrigin.split("://")[1];
const currentHostname = currentOrigin.split("://")[1].split(":")[0];
const currentPort = currentOrigin.split("://")[1].split(":")[1] || "443"; // FIXME: Handle http port
const currentProtocol = currentOrigin.split("/")[0];

export var mantalonWorker: ServiceWorker;
navigator.serviceWorker.ready.then((registration) => {
    if (registration.active) {
        mantalonWorker = registration.active;
    }
});

navigator.serviceWorker.addEventListener("message", event => {
    if (event.data.type === "mantalon-update-client-cookies") {
        cookies = event.data.cookies;
    } else if (event.data.type.startsWith("mantalon-")) {
        console.log("Received message from Mantalon", event.data);
    }
});


export function getAllPropertyNames(obj): Set<string> {
    const props: Set<string> = new Set();
    let current = obj;
    
    while (current && current !== Object.prototype) {
        Object.getOwnPropertyNames(current).forEach(prop => props.add(prop));
        current = Object.getPrototypeOf(current);
    }
    
    return props;
}

// HTMLIframeElement redefinitions

const originalSrcDescriptor = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, "src");

Object.defineProperty(HTMLIFrameElement.prototype, "src", {
    set(value) {
        let realUrl = fromFakeUrl(value, location.protocol, location.host, location.port).href;
        // console.warn(`Setting src to ${value} (${realUrl})`);

        if (originalSrcDescriptor && originalSrcDescriptor.set) {
            originalSrcDescriptor.set.call(this, realUrl);
        } else {
            this.setAttribute("src", realUrl);
        }
    },
    get() {
        let value = originalSrcDescriptor && originalSrcDescriptor.get ? originalSrcDescriptor.get.call(this) : this.getAttribute("src");
        let fakeUrl = intoFakeUrl(value, currentProtocol, currentHost, currentPort).href;
        // console.warn(`Getting src: ${value} (${fakeUrl})`);

        return fakeUrl;
    },
    configurable: true,
    enumerable: true,
});

// Worker redefinitions

const OriginalWorker = Worker;

(window as any).Worker = function (scriptURL: string | URL, options?: WorkerOptions): Worker {
    const realUrl = fromFakeUrl(scriptURL.toString(), location.protocol, location.host, location.port).href;
    console.warn(`Creating Worker with scriptURL: ${scriptURL} (rewritten to ${realUrl})`);

    return new OriginalWorker(realUrl, options);
} as any as typeof Worker;

// Proxies

const proxiedLocation = makeProxiedLocation(
    window.location,
    currentHost,
    currentHostname,
    currentOrigin,
    currentProtocol,
    currentPort,
    targetOrigins
);
const proxiedDocument = makeProxiedDocument(
    document,
    cookies,
    proxiedLocation
);
const proxiedWindow = makeProxiedWindow(
    window,
    targetOrigins,
    proxiedDocument,
    proxiedLocation
);

globalThis.proxiedLocation = proxiedLocation;
globalThis.proxiedDocument = proxiedDocument;
globalThis.proxiedWindow = proxiedWindow;
