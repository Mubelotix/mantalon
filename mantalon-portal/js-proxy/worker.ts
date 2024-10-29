import { getAllPropertyNames } from "./main";
import { makeProxiedMessageEvent } from "./message";

export function makeProxiedDedicatedWorker(realWorker, fakeLocation) {
    const workerInitialMethods = getAllPropertyNames(realWorker);

    let fakeWorker;
    const realLocation = realWorker.location;
    const workerHandler = {
        get(realWorker, prop, receiver) {
            if (prop === "location") {
                return fakeLocation;
            }
            if (prop === "origin") {
                return fakeLocation.origin;
            }
            if (prop === "self") {
                return fakeWorker;
            }
            if (prop === "postMessage") {
                console.warn("postMessage is not implemented (worker)");
                // return function (message, fakeTargetOrigin, transfer) {
                //     let realTargetOrigin = fromFakeUrl(fakeTargetOrigin, realLocation.protocol, realLocation.hostname, realLocation.port).origin;
                //     console.log(`postMessage: ${message} to ${fakeTargetOrigin} (${realTargetOrigin})`);
                //     return realWindow.postMessage({
                //         actualMessage: message,
                //         fakeOrigin: fakeLocation.origin,
                //     }, realTargetOrigin, transfer);
                // };
            }
            if (prop === "addEventListener") {
                console.warn("addEventListener is not implemented (worker)");
                // return function (type, listener, options) {
                //     if (type === "message") {
                //         let listenerWrapper = function (event: MessageEvent) {
                //             if (event.origin === realLocation.origin) {
                //                 let actualMessage = event.data.actualMessage;
                //                 let fakeOrigin = event.data.fakeOrigin;
                //                 return listener(makeProxiedMessageEvent(event, actualMessage, fakeOrigin));
                //             }
                //             return listener(event);
                //         }
                //         return realWindow.addEventListener(type, listenerWrapper, options);
                //     } else {
                //         return realWindow.addEventListener(type, listener, options);
                //     }
                // };
            }
            if (prop === "onmessage" || prop === "onmessageerror" || prop === "removeEventListener") {
                console.warn(prop + " (get) is not implemented: page might detect the proxy");
            }
    
            const value = Reflect.get(realWorker, prop);
            if (typeof value === "function" && workerInitialMethods.has(prop) && prop[0] !== prop[0].toUpperCase()) {
                return value.bind(realWorker);
            }
            return value;
        },
    
        set(realWorker, prop, value, receiver) {
            if (prop === "location" || prop === "onmessage" || prop === "onmessageerror") {
                console.warn(prop + " (set) is not implemented: page might detect the proxy");
            }
    
            if (typeof value === "function" && workerInitialMethods.has(prop) && prop[0] !== prop[0].toUpperCase()) {
                return Reflect.set(realWorker, prop, value.bind(realWorker));
            }
            return Reflect.set(realWorker, prop, value);
        }
    };
    
    fakeWorker = new Proxy(realWorker, workerHandler);
    return fakeWorker;

}

export function setupWorkerPostMessage(worker) {
    worker.realAddEventListener = worker.addEventListener;
    const realLocation = worker.location;

    worker.addEventListener = function (type, listener, options) {
        if (type === "message") {
            let listenerWrapper = function (event: MessageEvent) {
                if (event.origin === realLocation.origin) {
                    let actualMessage = event.data.actualMessage;
                    let fakeOrigin = event.data.fakeOrigin;
                    console.log(`Worker received message from ${fakeOrigin}: ${actualMessage}`);
                    return listener(makeProxiedMessageEvent(event, actualMessage, fakeOrigin));
                } else {
                    console.warn(`Message from unexpected origin: ${event.origin}`);
                }
                return listener(event);
            }
            return worker.realAddEventListener(type, listenerWrapper, options);
        } else {
            return worker.realAddEventListener(type, listener, options);
        }
    };;
}
