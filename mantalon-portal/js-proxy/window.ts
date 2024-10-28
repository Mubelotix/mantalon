import { makeProxiedDocument } from "./document";
import { fromFakeUrl, makeProxiedLocation } from "./location";
import { getAllPropertyNames } from "./main";
import { makeProxiedMessageEvent } from "./message";

export function makeProxiedWindow(
    realWindow: Window,
    targetOrigins: Set<string>,
    fakeDocument,
    fakeLocation,
) {
    const windowInitialMethods = getAllPropertyNames(realWindow);

    let fakeWindow;
    const realLocation = realWindow.location;
    const windowHandler = {
        get(realWindow, prop, receiver) {
            if (prop === "document") {
                return fakeDocument;
            }
            if (prop === "location") {
                return fakeLocation;
            }
            if (prop === "window" || prop === "self") {
                return fakeWindow;
            }
            if (prop === "parent" || prop === "top") {
                let realParentWindow = prop === "parent" ? realWindow.parent : realWindow.top;
                if (
                    typeof realParentWindow === 'object'
                    && realParentWindow !== null
                    && typeof realParentWindow.window === 'object'
                    && realParentWindow.window === realParentWindow
                ) {
                    let realParentLocation = realParentWindow.location;
                    let realParentDocument = realParentWindow.document;
                    let fakeParentLocation = makeProxiedLocation(realParentLocation, fakeLocation.origin, fakeLocation.hostname, fakeLocation.origin, fakeLocation.protocol, fakeLocation.port, targetOrigins); // FIXME: This is not correct, you could very well expect the parent to be in a different origin
                    let fakeParentDocument = makeProxiedDocument(realParentDocument, fakeDocument.cookie, targetOrigins, fakeParentLocation);
                    return makeProxiedWindow(realParentWindow, targetOrigins, fakeParentDocument, fakeParentLocation);
                } else {
                    console.error(`Parent window is not an instance of Window: ${realParentWindow}`);
                }
            }
            if (prop === "postMessage") {
                return function (message, fakeTargetOrigin, transfer) {
                    let realTargetOrigin = fromFakeUrl(fakeTargetOrigin, realLocation.protocol, realLocation.hostname, realLocation.port).origin;
                    console.log(`postMessage: ${message} to ${fakeTargetOrigin} (${realTargetOrigin})`);
                    return realWindow.postMessage({
                        actualMessage: message,
                        fakeOrigin: fakeLocation.origin,
                    }, realTargetOrigin, transfer);
                };
            }
            if (prop === "addEventListener") {
                return function (type, listener, options) {
                    if (type === "message") {
                        let listenerWrapper = function (event: MessageEvent) {
                            if (event.origin === realLocation.origin) {
                                let actualMessage = event.data.actualMessage;
                                let fakeOrigin = event.data.fakeOrigin;
                                return listener(makeProxiedMessageEvent(event, actualMessage, fakeOrigin));
                            }
                            return listener(event);
                        }
                        return realWindow.addEventListener(type, listenerWrapper, options);
                    } else {
                        return realWindow.addEventListener(type, listener, options);
                    }
                };
            }
            if (prop === "cookieStore" || prop === "onmessage" || prop === "onmessageerror" || prop === "removeEventListener") {
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
                fakeLocation.href = value;
                return true;
            }
            if (prop === "cookieStore" || prop === "onmessage" || prop === "onmessageerror") {
                console.warn(prop + " (set) is not implemented: page might detect the proxy");
            }
    
            if (typeof value === "function" && windowInitialMethods.has(prop)) {
                return Reflect.set(realWindow, prop, value.bind(realWindow));
            }
            return Reflect.set(realWindow, prop, value);
        }
    };
    
    fakeWindow = new Proxy(realWindow, windowHandler);
    return fakeWindow;
}
