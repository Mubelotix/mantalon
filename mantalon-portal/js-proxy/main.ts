// TODO: Message passing
// TODO: window.cookieStore
// TODO: location
// TODO: iframes
// TODO: scripts

const currentOrigin = "init-origin"; // Value is added automatically when the script gets injected
const targetOrigins = new Set(["init-targetOrigins"]); // Value is added automatically when the script gets injected
var cookies = "init-cookies"; // Value is added automatically when the script gets injected
const currentHost = currentOrigin.split("://")[1];
const currentHostname = currentOrigin.split("://")[1].split(":")[0];
const currentPort = currentOrigin.split("://")[1].split(":")[1] || "443"; // FIXME: Handle http port
const currentProtocol = currentOrigin.split("/")[0];

const fakeOrigin = new URL(currentOrigin);

export var mantalonWorker: ServiceWorker; // TODO: not available in workers

declare global {
    var mantalon: (v: any) => any;
}

function mantalon(v: any): any {
    return v;
}

globalThis.mantalon = mantalon;

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
} else if (globalThis instanceof DedicatedWorkerGlobalScope) {
    console.error("Worker proxying not yet implemented");
} else {
    console.error(`Unsupported environment: ${self}`);
}
