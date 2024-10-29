import { fromFakeUrl, intoFakeUrl } from "../location";

export function setupIframes(fakeProtocol: string, fakeHost: string, fakePort: string) {
    const originalSrcDescriptor = Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype, "src");

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
            let fakeUrl = intoFakeUrl(value, fakeProtocol, fakeHost, fakePort).href;
            // console.warn(`Getting src: ${value} (${fakeUrl})`);

            return fakeUrl;
        },
        configurable: true,
        enumerable: true,
    });
}

// TODO: contentWindow
