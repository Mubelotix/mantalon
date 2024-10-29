import { makeProxiedLocation } from "./location";
import { getAllPropertyNames, mantalonWorker } from "./main";
import { makeProxiedWindow } from "./window";


export function makeProxiedDocument(
    realDocument: Document,
    cookies: string,
    targetOrigins: Set<string>,
    fakeLocation,
) {
    const documentInitialMethods = getAllPropertyNames(realDocument);

    const documentHandler = {
        get(realDocument, prop, receiver) {
            if (prop === "location") {
                return fakeLocation;
            }
            if (prop === "cookie") {
                return cookies;
            }
            if (prop === "URL" || prop === "documentURI" || prop === "baseURI") {
                return fakeLocation.href;
            }
            if (prop === "referrer") {
                return fakeLocation.origin;
            }
            if (prop === "domain") {
                return fakeLocation.hostname;
            }
            if (prop === "defaultView") {
                console.warn("defaultView is badly implemented. Returning the global");
                return globalThis;
            }

            const value = Reflect.get(realDocument, prop);
            if (typeof value === "function" && documentInitialMethods.has(prop)) {
                return value.bind(realDocument);
            }
            return value;
        },

        set(realDocument, prop, value, receiver): boolean {
            if (prop === "location") {
                fakeLocation.href = value;
                return true;
            }
            if (prop === "cookie") {
                mantalonWorker.postMessage({
                    type: "mantalon-update-sw-cookie",
                    href: fakeLocation.href,
                    cookie: value
                });
                return true;
            }
            if (prop === "URL" || prop == "documentURI" || prop === "baseURI" || prop === "referer" || prop === "domain") {
                console.warn(prop + " (set) is not implemented: page might detect the proxy");
            }

            if (typeof value === "function" && documentInitialMethods.has(prop)) {
                return Reflect.set(realDocument, prop, value.bind(realDocument));
            }
            return Reflect.set(realDocument, prop, value);
        }
    };
    
    return new Proxy(realDocument, documentHandler);
}
