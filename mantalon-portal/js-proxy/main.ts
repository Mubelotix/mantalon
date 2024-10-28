// TODO: Message passing
// TODO: History
// TODO: Parent window

interface Window {
    proxiedWindow: typeof proxiedWindow;
    proxiedDocument: typeof proxiedDocument;
    proxiedLocation: typeof proxiedLocation;
}

const currentOrigin = "origin"; // Value is added automatically when the script gets injected
const clientId = "clientId"; // Value is added automatically when the script gets injected
const targetOrigins = new Set(["targetOrigins"]); // Value is added automatically when the script gets injected
var cookies = "cookies"; // Value is added automatically when the script gets injected
const currentHost = currentOrigin.split("://")[1];
const currentHostname = currentOrigin.split("://")[1].split(":")[0];
const currentPort = currentOrigin.split("://")[1].split(":")[1] || "443"; // FIXME: Handle http port
const currentProtocol = currentOrigin.split("/")[0];

var mantalonWorker: ServiceWorker;
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

function getFakedUrl(): URL {
    let fakedUrl = new URL(window.location.href);
    fakedUrl.protocol = currentProtocol;
    fakedUrl.host = currentHost;
    fakedUrl.port = currentPort;
    return fakedUrl;
}

async function setFakedUrl(target: URL) {
    if (!mantalonWorker) {
        throw new Error("Can't navigate until Mantalon is initialized");
    }

    mantalonWorker.postMessage({
        type: "mantalon-change-origin",
        origin: target.origin,
        clientId: clientId
    });

    let waitResponse = new Promise((resolve, reject) => {
        function handleResponse(event) {
            if (event.data && event.data.type === "mantalon-change-origin-success") {
                mantalonWorker.removeEventListener("message", handleResponse);
                resolve(event.data);
            } else if (event.data && event.data.type === "mantalon-change-origin-failure") {
                mantalonWorker.removeEventListener("message", handleResponse);
                reject(new Error("Failed to change origin"));
            }
        }

        mantalonWorker.addEventListener("message", handleResponse);
    });

    await waitResponse;
    
    target.protocol = window.location.protocol;
    target.hostname = window.location.hostname;
    target.port = window.location.port;
    window.location.href = target.toString();
}

function getAllPropertyNames(obj): Set<string> {
    const props: Set<string> = new Set();
    let current = obj;
    
    while (current && current !== Object.prototype) {
        Object.getOwnPropertyNames(current).forEach(prop => props.add(prop));
        current = Object.getPrototypeOf(current);
    }
    
    return props;
}

// The location proxy

const LOCATION_WHITELISTED: Set<string> = new Set(["hash", "pathname", "search", "reload", "toString"]);

const locationInitialMethods = getAllPropertyNames(location);

const locationHandler = {
    get(targetLocation, prop, receiver) {
        if (LOCATION_WHITELISTED.has(prop)) {
            const value = Reflect.get(targetLocation, prop);
            if (typeof value === 'function' && locationInitialMethods.has(prop)) {
                return value.bind(targetLocation);
            }
            return value;
        }

        switch (prop) {
            case "ancestorOrigins":
                console.error("ancestorOrigins is not implemented. Returning empty array."); // FIXME: Implement ancestorOrigins
                return [];
            case "host":
                return currentHost;
            case "hostname":
                return currentHostname;
            case "href":
                let targetHref = targetLocation.href;
                let targetOrigin = targetLocation.origin;
                return currentOrigin + targetHref.substring(targetOrigin.length);
            case "origin":
                return currentOrigin;
            case "port":
                return currentPort;
            case "protocol":
                return currentProtocol;
            case "assign":
                return function (url) {
                    let fakedUrl = new URL(url.toString());
                    if (targetOrigins.has(fakedUrl.origin)) {
                        setFakedUrl(fakedUrl);
                        return true;
                    }
                    return targetLocation.assign(url);
                };
            case "replace":
                return function (url) {
                    let fakedUrl = new URL(url.toString());
                    if (targetOrigins.has(fakedUrl.origin)) {
                        setFakedUrl(fakedUrl);
                        return true;
                    }
                    return targetLocation.replace(url);
                };
        }
        
        return undefined;
    },

    set(targetLocation, prop, value, receiver): boolean {
        if (LOCATION_WHITELISTED.has(prop)) {
            if (typeof value === 'function' && locationInitialMethods.has(prop)) {
                return Reflect.set(targetLocation, prop, value.bind(targetLocation));
            }
            return Reflect.set(targetLocation, prop, value);
        }

        switch (prop) {
            case "host":
                if (value === currentHost) {
                    return true;
                }
                if (targetOrigins.has(currentProtocol + "//" + value)) {
                    let fakedUrl = getFakedUrl();
                    fakedUrl.host = value;
                    setFakedUrl(fakedUrl);
                    return true;
                }
                return Reflect.set(targetLocation, "host", currentHost);
            case "hostname":
                if (value === currentHostname) {
                    return true;
                }
                if (targetOrigins.has(currentProtocol + "://" + value + (currentPort ? ":" + currentPort : ""))) {
                    let fakedUrl = getFakedUrl();
                    fakedUrl.hostname = value;
                    setFakedUrl(fakedUrl);
                    return true;
                }
                return Reflect.set(targetLocation, "hostname", currentHostname);
            case "href":
                if (value === currentOrigin) {
                    return true;
                }
                let fakedUrl = new URL(value);
                if (targetOrigins.has(fakedUrl.origin)) {
                    setFakedUrl(fakedUrl);
                    return true;
                }
                return Reflect.set(targetLocation, "href", currentOrigin);
            case "port":
                if (value === currentPort) {
                    return true;
                }
                if (targetOrigins.has(currentProtocol + "//" + currentHostname + ":" + value)) { // TODO: Handle special port cases
                    let fakedUrl = getFakedUrl();
                    fakedUrl.port = value;
                    setFakedUrl(fakedUrl);
                    return true;
                }
                return Reflect.set(targetLocation, "port", currentPort);
            case "protocol":
                if (value === currentProtocol) {
                    return true;
                }
                if (targetOrigins.has(value + "://" + currentHostname + (currentPort ? ":" + currentPort : ""))) {
                    let fakedUrl = getFakedUrl();
                    fakedUrl.protocol = value;
                    setFakedUrl(fakedUrl);
                    return true;
                }
                return Reflect.set(targetLocation, "protocol", currentProtocol);
        }

        return false
    }
}
const proxiedLocation = new Proxy(location, locationHandler);

// The document proxy

const documentInitialMethods = getAllPropertyNames(document);

const documentHandler = {
    get(targetDocument, prop, receiver) {
        if (prop === "location") {
            return proxiedLocation;
        }
        if (prop === "cookie") {
            return cookies;
        }
        if (prop === "URL" || prop === "documentURI" || prop === "baseURI") {
            return getFakedUrl().href;
        }
        if (prop === "referrer") {
            return currentOrigin;
        }
        if (prop === "domain") {
            return currentHostname;
        }

        const value = Reflect.get(targetDocument, prop);
        if (typeof value === 'function' && documentInitialMethods.has(prop)) {
            return value.bind(targetDocument);
        }
        return value;
    },

    set(targetDocument, prop, value, receiver): boolean {
        if (prop === "location") {
            setFakedUrl(value);
            return true;
        }
        if (prop === "cookie") {
            mantalonWorker.postMessage({
                type: "mantalon-update-sw-cookie",
                href: getFakedUrl().href,
                cookie: value
            });
            return true;
        }
        if (prop === "URL" || prop == "documentURI" || prop === "baseURI" || prop === "referer" || prop === "domain") {
            console.warn(prop + " (set) is not implemented: page might detect the proxy");
        }

        if (typeof value === 'function' && documentInitialMethods.has(prop)) {
            return Reflect.set(targetDocument, prop, value.bind(targetDocument));
        }
        return Reflect.set(targetDocument, prop, value);
    }
};
const proxiedDocument = new Proxy(document, documentHandler);

// The window proxy

const windowInitialMethods = getAllPropertyNames(window);

const windowHandler = {
    get(targetWindow, prop, receiver) {
        if (prop === "document") {
            return proxiedDocument;
        }
        if (prop === "location") {
            return proxiedLocation;
        }
        if (prop === "history" || prop === "postMessage" || prop === "parent") {
            console.warn(prop + " (get) is not implemented: page might detect the proxy");
        }

        const value = Reflect.get(targetWindow, prop);
        if (typeof value === 'function' && windowInitialMethods.has(prop)) {
            return value.bind(targetWindow);
        }
        return value;
    },

    set(targetWindow, prop, value, receiver) {
        if (prop === "location") {
            setFakedUrl(value);
            return true;
        }
        if (prop === "history" || prop === "postMessage" || prop === "parent") {
            console.warn(prop + " (set) is not implemented: page might detect the proxy");
        }

        if (typeof value === 'function' && windowInitialMethods.has(prop)) {
            return Reflect.set(targetWindow, prop, value.bind(targetWindow));
        }
        return Reflect.set(targetWindow, prop, value);
    }
};

const proxiedWindow = new Proxy(window, windowHandler);

window.proxiedWindow = proxiedWindow;
window.proxiedDocument = proxiedDocument;
window.proxiedLocation = proxiedLocation;
