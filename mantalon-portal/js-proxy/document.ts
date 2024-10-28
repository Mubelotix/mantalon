import { makeProxiedLocation } from "./location";
import { getAllPropertyNames, mantalonWorker } from "./main";


export function makeProxiedDocument(
    realDocument: Document,
    cookies: string,
    proxiedLocation,
) {
    const documentInitialMethods = getAllPropertyNames(realDocument);

    const documentHandler = {
        get(realDocument, prop, receiver) {
            if (prop === "location") {
                return proxiedLocation;
            }
            if (prop === "cookie") {
                return cookies;
            }
            if (prop === "URL" || prop === "documentURI" || prop === "baseURI") {
                return proxiedLocation.href;
            }
            if (prop === "referrer") {
                return proxiedLocation.origin;
            }
            if (prop === "domain") {
                return proxiedLocation.hostname;
            }
            if (prop === "defaultView") {
                console.warn("defaultView is not implemented: page might detect the proxy");
            }

            const value = Reflect.get(realDocument, prop);
            if (typeof value === "function" && documentInitialMethods.has(prop)) {
                return value.bind(realDocument);
            }
            return value;
        },

        set(realDocument, prop, value, receiver): boolean {
            if (prop === "location") {
                proxiedLocation.href = value;
                return true;
            }
            if (prop === "cookie") {
                mantalonWorker.postMessage({
                    type: "mantalon-update-sw-cookie",
                    href: proxiedLocation.href,
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
