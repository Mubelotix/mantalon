// TODO: Message passing
// TODO: window.cookieStore

import { makeProxiedDocument } from "./document";
import { setupIframes } from "./simple/iframe";
import { fromFakeUrl, intoFakeUrl, makeProxiedLocation } from "./location";
import { makeProxiedWindow, setupWindowPostMessage } from "./window";
import { setupWorkers } from "./simple/worker";
import { makeProxiedDedicatedWorker, setupWorkerPostMessage } from "./worker";

const currentOrigin = "init-origin"; // Value is added automatically when the script gets injected
const targetOrigins = new Set(["init-targetOrigins"]); // Value is added automatically when the script gets injected
var cookies = "init-cookies"; // Value is added automatically when the script gets injected
const currentHost = currentOrigin.split("://")[1];
const currentHostname = currentOrigin.split("://")[1].split(":")[0];
const currentPort = currentOrigin.split("://")[1].split(":")[1] || "443"; // FIXME: Handle http port
const currentProtocol = currentOrigin.split("/")[0];

const fakeOrigin = new URL(currentOrigin);

export function getAllPropertyNames(obj): Set<string> {
    const props: Set<string> = new Set();
    let current = obj;
    
    while (current && current !== Object.prototype) {
        Object.getOwnPropertyNames(current).forEach(prop => props.add(prop));
        current = Object.getPrototypeOf(current);
    }
    
    return props;
}

export var mantalonWorker: ServiceWorker; // TODO: not available in workers

if (typeof window !== "undefined" && globalThis instanceof window.Window) {
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
    
    setupIframes(fakeOrigin);
    setupWindowPostMessage(window);
    setupWorkers();
    
    const proxiedLocation = makeProxiedLocation(
        location,
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
        targetOrigins,
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
    globalThis.proxiedSelf = proxiedWindow;
    globalThis.proxiedGlobalThis = proxiedWindow;
    
    globalThis = proxiedWindow;
    self = proxiedWindow;
} else if (globalThis instanceof DedicatedWorkerGlobalScope) {
    setupWorkers();
    setupWorkerPostMessage(self);
    
    const proxiedLocation = makeProxiedLocation(
        location,
        currentHost,
        currentHostname,
        currentOrigin,
        currentProtocol,
        currentPort,
        targetOrigins
    );
    const proxiedDedicatedWorker = makeProxiedDedicatedWorker(
        globalThis,
        proxiedLocation
    );
    
    globalThis.proxiedLocation = proxiedLocation;
    globalThis.proxiedSelf = proxiedDedicatedWorker;
    globalThis.proxiedGlobalThis = proxiedDedicatedWorker;

    globalThis = proxiedDedicatedWorker;
    self = proxiedDedicatedWorker;
} else {
    console.error(`Unsupported environment: ${self}`);
}
