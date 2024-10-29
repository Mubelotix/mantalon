import { makeProxiedDocument } from "../document";
import { fromFakeUrl, intoFakeUrl, makeProxiedLocation } from "../location";

export function setupIframes(fakeOrigin: URL) {
    const originalSrcDescriptor = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, "src");
    const originalContentWindowDescriptor = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, "contentWindow");

    Object.defineProperty(HTMLIFrameElement.prototype, "src", {
        set(value) {
            let realUrl = fromFakeUrl(value, location.protocol, location.host, location.port).href;
            // console.warn(`Setting src to ${value} (${realUrl})`);

            if (originalSrcDescriptor && originalSrcDescriptor.set) {
                originalSrcDescriptor.set.call(this, realUrl);
            } else {
                this.setAttribute("src", realUrl);
            }
        },
        get() {
            let value = originalSrcDescriptor && originalSrcDescriptor.get ? originalSrcDescriptor.get.call(this) : this.getAttribute("src");
            let fakeUrl = intoFakeUrl(value, fakeOrigin.protocol, fakeOrigin.host, fakeOrigin.port).href;
            // console.warn(`Getting src: ${value} (${fakeUrl})`);

            return fakeUrl;
        },
        configurable: true,
        enumerable: true,
    });

    Object.defineProperty(HTMLIFrameElement.prototype, "contentWindow", {
        get() {
            let realWindow = originalContentWindowDescriptor && originalContentWindowDescriptor.get ? originalContentWindowDescriptor.get.call(this) : null;
            if (!realWindow) {
                console.log("contentWindow is null");
                return null;
            }

            if (realWindow.proxiedWindow) {
                console.info("ProxiedWindow found in iframe");
                return realWindow.proxiedWindow;
            }

            console.warn("ProxiedWindow not found in iframe, returning the real one"); // I expect this to happen on cross-origin iframes

            return realWindow;
        },
        configurable: true,
        enumerable: true,
    });

    Object.defineProperty(HTMLIFrameElement.prototype, "contentDocument", {
        get() {
            let contentWindow = this.contentWindow;
            if (!contentWindow) {
                return null;
            }
            return contentWindow.document;
        }
    });
}
