// Replicates the window.location object but changes the host
class FakeLocation {
    constructor(host) {
        this._host = host;
    }

    get href() {
        return "https://" + this._host + window.location.pathname + window.location.search + window.location.hash;
    }

    get protocol() {
        return "https:"
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
let global = (typeof window !== 'undefined') ? window : self;
global.fakeLocation = new FakeLocation("app.dev.insaplace.me");

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
        this._worker.onmessage = function(event) {
            console.log("Received message from worker", event);
        }.bind(this);
        this._worker.onerror = function(event) {
            console.log("Received error from worker", event);
        }.bind(this);
        this._worker.onmessageerror = function(event) {
            console.log("Received message error from worker", event);
        }.bind(this);
    }

    postMessage(message) {
        console.log("Sending message to worker", message);
        this._worker.postMessage(message);
    }

    terminate() {
        this._worker.terminate();
    }
}

// Replicate the MessageChannel class
class FakeMessageChannel {
    constructor() {
        this._channel = new MessageChannel();
        this._channel.port1.addEventListener("message", function(event) {
            console.log("Received message from port1", event, global);
        });
        this._channel.port2.addEventListener("message", function(event) {
            console.log("Received message from port2", event, global);
        });
        this._channel.port1.addEventListener("messageerror", function(event) {
            console.log("Received message error from port1", event, global);
        });
        this._channel.port2.addEventListener("messageerror", function(event) {
            console.log("Received message error from port2", event, global);
        });
        this._channel.port1.start();
        this._channel.port2.start();
    }

    get port1() {
        return this._channel.port1;
    }

    get port2() {
        return this._channel.port2;
    }
}

// Replicates postMessage
global.fakePostMessage = function(message, targetOrigin, transfer) {
    if (targetOrigin.includes("app.dev.insaplace.me")) {
        targetOrigin = "http://localhost:8000"
    }
    console.log("Sending message to", targetOrigin, message, transfer);
    global.postMessage(message, "*", transfer);
};
global.addEventListener("message", function(event) {
    console.log("Received message", event);
});
