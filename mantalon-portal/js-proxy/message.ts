import { getAllPropertyNames } from "./main";

export function makeProxiedMessageEvent(realEvent: MessageEvent, actualData: any, fakeOrigin: string) {
    const messageInitialMethods = getAllPropertyNames(realEvent);

    const eventHandler = {
        get(realEvent, prop, receiver) {
            if (prop === "origin") {
                return fakeOrigin;
            }

            if (prop === "data") {
                return actualData;
            }

            const value = Reflect.get(realEvent, prop);
            if (typeof value === "function" && messageInitialMethods.has(prop)) {
                return value.bind(realEvent);
            }
            return value;
        },

        set(realEvent, prop, value, receiver): boolean {
            if (typeof value === "function" && messageInitialMethods.has(prop)) {
                return Reflect.set(realEvent, prop, value.bind(realEvent));
            }
            return Reflect.set(realEvent, prop, value);
        }
    }

    return new Proxy(realEvent, eventHandler);

}