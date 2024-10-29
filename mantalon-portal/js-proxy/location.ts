import { getAllPropertyNames } from "./main";

const LOCATION_WHITELISTED: Set<string> = new Set(["hash", "pathname", "search", "reload", "toString"]);

export function intoFakeUrl(realUrl: string | URL, fakeProtocol: string, fakeHost: string, fakePort: string): URL {
    let fakedUrl = typeof realUrl === "string" ? new URL(realUrl) : new URL(realUrl.toString());
    fakedUrl.protocol = fakeProtocol;
    fakedUrl.host = fakeHost;
    fakedUrl.port = fakePort;
    return fakedUrl;
}

// FIXME: This only works when the realURL is on the active target and it's a target origin
export function fromFakeUrl(fakedUrl: string | URL, realProtocol: string, realHost: string, realPort: string): URL {
    let realUrl = typeof fakedUrl === "string" ? new URL(fakedUrl) : new URL(fakedUrl.toString());
    realUrl.protocol = window.location.protocol;
    realUrl.hostname = window.location.hostname;
    realUrl.port = window.location.port; // TODO: verify it works when port is empty
    return realUrl;
}

export function makeProxiedLocation(
    realLocation: Location,
    fakeHost: string,
    fakeHostname: string,
    fakeOrigin: string,
    fakeProtocol: string,
    fakePort: string,
    targetOrigins: Set<string>
) {
    const locationInitialMethods = getAllPropertyNames(realLocation);

    const locationHandler = {
        get(realLocation, prop, receiver) {
            if (LOCATION_WHITELISTED.has(prop)) {
                const value = Reflect.get(realLocation, prop);
                if (typeof value === "function" && locationInitialMethods.has(prop)) {
                    return value.bind(realLocation);
                }
                return value;
            }

            switch (prop) {
                case "ancestorOrigins":
                    console.error("ancestorOrigins is not implemented. Returning empty array."); // FIXME: Implement ancestorOrigins
                    return [];
                case "host":
                    return fakeHost;
                case "hostname":
                    return fakeHostname;
                case "href":
                    let realHref = realLocation.href;
                    let realOrigin = realLocation.origin;
                    return fakeOrigin + realHref.substring(realOrigin.length);
                case "origin":
                    return fakeOrigin;
                case "port":
                    return fakePort;
                case "protocol":
                    return fakeProtocol;
                case "assign":
                    return function (fakeUrl) {
                        let realUrl = fromFakeUrl(fakeUrl, realLocation.protocol, realLocation.hostname, realLocation.port);
                        return realLocation.assign(realUrl);
                    };
                case "replace":
                    return function (url) {
                        let realUrl = fromFakeUrl(url, realLocation.protocol, realLocation.hostname, realLocation.port);
                        return realLocation.replace(realUrl);
                    };
            }

            console.error(`Location property ${prop} is not implemented`);

            return undefined;
        },

        set(realLocation, prop, value, receiver): boolean {
            if (LOCATION_WHITELISTED.has(prop)) {
                if (typeof value === "function" && locationInitialMethods.has(prop)) {
                    return Reflect.set(realLocation, prop, value.bind(realLocation));
                }
                return Reflect.set(realLocation, prop, value);
            }

            switch (prop) {
                case "host":
                    if (value === fakeHost) {
                        return true;
                    }
                    if (targetOrigins.has(fakeProtocol + "//" + value)) {
                        fakeHost = value;
                        return true;
                    }
                    return Reflect.set(realLocation, "host", fakeHost);
                case "hostname":
                    if (value === fakeHostname) {
                        return true;
                    }
                    if (targetOrigins.has(fakeProtocol + "://" + value + (fakePort ? ":" + fakePort : ""))) {
                        fakeHostname = value;
                        return true;
                    }
                    return Reflect.set(realLocation, "hostname", fakeHostname);
                case "href":
                    if (value === fakeOrigin) {
                        return true;
                    }
                    let fakedUrl = new URL(value);
                    if (targetOrigins.has(fakedUrl.origin)) {
                        let realUrl = fromFakeUrl(fakedUrl, realLocation.protocol, realLocation.hostname, realLocation.port);
                        realLocation.href = realUrl.href;
                        return true;
                    }
                    return Reflect.set(realLocation, "href", fakeOrigin);
                case "port":
                    if (value === fakePort) {
                        return true;
                    }
                    if (targetOrigins.has(fakeProtocol + "//" + fakeHostname + ":" + value)) { // TODO: Handle special port cases
                        fakePort = value;
                        return true;
                    }
                    return Reflect.set(realLocation, "port", fakePort);
                case "protocol":
                    if (value === fakeProtocol) {
                        return true;
                    }
                    if (targetOrigins.has(value + "://" + fakeHostname + (fakePort ? ":" + fakePort : ""))) {
                        fakeProtocol = value;
                        return true;
                    }
                    return Reflect.set(realLocation, "protocol", fakeProtocol);
            }

            console.error(`Location property ${prop} is not implemented (set)`);

            return false
        }
    }

    return new Proxy(realLocation, locationHandler);
}
