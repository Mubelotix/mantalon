import { makeProxiedDocument } from "./document";
import { makeProxiedLocation } from "./location";
import { getAllPropertyNames } from "./main";

export function makeProxiedWindow(
    realWindow: Window,
    targetOrigins: Set<string>,
    proxiedDocument,
    proxiedLocation,
) {
    const windowInitialMethods = getAllPropertyNames(realWindow);

    let proxiedWindow;
    const windowHandler = {
        get(realWindow, prop, receiver) {
            if (prop === "document") {
                return proxiedDocument;
            }
            if (prop === "location") {
                return proxiedLocation;
            }
            if (prop === "window") {
                return proxiedWindow;
            }
            if (prop === "parent") {
                let realParentWindow = realWindow.parent.window;
                if (
                    typeof realParentWindow === 'object'
                    && realParentWindow !== null
                    && typeof realParentWindow.window === 'object'
                    && realParentWindow.window === realParentWindow
                ) {
                    let realParentLocation = realParentWindow.location;
                    let realParentDocument = realParentWindow.document;
                    let fakeParentLocation = makeProxiedLocation(realParentLocation, proxiedLocation.origin, proxiedLocation.hostname, proxiedLocation.origin, proxiedLocation.protocol, proxiedLocation.port, targetOrigins); // FIXME: This is not correct, you could very well expect the parent to be in a different origin
                    let fakeParentDocument = makeProxiedDocument(realParentDocument, proxiedDocument.cookie, fakeParentLocation);
                    return makeProxiedWindow(realParentWindow, targetOrigins, fakeParentDocument, fakeParentLocation);
                } else {
                    console.error(`Parent window is not an instance of Window: ${realParentWindow}`);
                }
            }
            if (prop === "postMessage" || prop === "parent" || prop == "top" || prop === "cookieStore") {
                console.warn(prop + " (get) is not implemented: page might detect the proxy");
            }
    
            const value = Reflect.get(realWindow, prop);
            if (typeof value === "function" && windowInitialMethods.has(prop)) {
                return value.bind(realWindow);
            }
            return value;
        },
    
        set(realWindow, prop, value, receiver) {
            if (prop === "location") {
                proxiedLocation.href = value;
                return true;
            }
            if (prop === "postMessage" || prop === "parent" || prop == "top" || prop === "cookieStore") {
                console.warn(prop + " (set) is not implemented: page might detect the proxy");
            }
    
            if (typeof value === "function" && windowInitialMethods.has(prop)) {
                return Reflect.set(realWindow, prop, value.bind(realWindow));
            }
            return Reflect.set(realWindow, prop, value);
        }
    };
    
    proxiedWindow = new Proxy(window, windowHandler);
    return proxiedWindow;
}
