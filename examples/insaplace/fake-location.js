// Replicates the window.location object but changes the host
class FakeLocation {
    constructor(host) {
        this._host = host;
    }

    get href() {
        return window.location.protocol + "//" + this._host + window.location.pathname + window.location.search + window.location.hash;
    }

    get protocol() {
        return window.location.protocol;
    }

    get host() {
        return this._host
    }

    get hostname() {
        return this._host.split(":")[0];
    }

    get port() {
        return this._host.split(":")[1] || "";
    }

    get pathname() {
        return window.location.pathname;
    }

    get search() {
        return window.location.search;
    }

    get hash() {
        return window.location.hash;
    }

    get origin() {
        return window.location.protocol + "//" + this._host;
    }
}
window.fakeLocation = new FakeLocation("app.dev.insaplace.me");

// Replicates the Worker class
class FakeWorker {
    constructor(url, options) {
        let newUrl = new URL(url);
        newUrl.searchParams.set('mantalon-protocol', newUrl.protocol);
        newUrl.searchParams.set('mantalon-host', newUrl.host);
        newUrl.searchParams.set('mantalon-navigate', "false");
        newUrl.protocol = "http:";
        newUrl.host = "localhost:8000";
        this._worker = new Worker(newUrl, options);
    }

    postMessage(message) {
        this._worker.postMessage(message);
    }

    terminate() {
        this._worker.terminate();
    }
}

// Replicates postMessage
window.oldPostMessage = window.postMessage;
window.postMessage = function(message, targetOrigin, transfer) {
    if (targetOrigin.includes("app.dev.insaplace.me")) {
        targetOrigin = "http://localhost:8000"
    }
    window.oldPostMessage(message, targetOrigin, transfer);
};
