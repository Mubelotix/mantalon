import { RequestDirection } from "./manifest";
import { manifest, orDefault } from "./sw";

export function addDefaultHeaders(headers: Headers, destination: RequestDestination, mode: RequestMode, currentOrigin: string) {
    if (!headers.has("accept")) {
        let acceptValue: string;
        switch (destination) {
            case "": {
                acceptValue = "*/*";
                break;
            }
            case "audio": {
                acceptValue = "audio/*, */*;q=0.1";
                break;
            }
            case "audioworklet": {
                acceptValue = "*/*";
                break;
            }
            case "document": {
                acceptValue = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8";
                break;
            }
            case "embed": {
                acceptValue = "*/*";
                break;
            }
            case "font": {
                acceptValue = "application/font-woff2;q=1.0,application/font-woff;q=0.9,*/*;q=0.8";
                break;
            }
            case "frame":
            case "iframe": {
                acceptValue = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
                break;
            }
            case "image": {
                acceptValue = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";
                break;
            }
            case "manifest": {
                acceptValue = "application/manifest+json,*/*;q=0.8";
                break;
            }
            case "object": {
                acceptValue = "*/*";
                break;
            }
            case "paintworklet": {
                acceptValue = "*/*";
                break;
            }
            case "report": {
                acceptValue = "*/*";
                break;
            }
            case "script": {
                acceptValue = "*/*";
                break;
            }
            case "sharedworker": {
                acceptValue = "*/*";
                break;
            }
            case "style": {
                acceptValue = "text/css,*/*;q=0.1";
                break;
            }
            case "track": {
                acceptValue = "*/*";
                break;
            }
            case "video": {
                acceptValue = "video/*, */*;q=0.1";
                break;
            }
            case "worker": {
                acceptValue = "*/*";
                break;
            }
            case "xslt": {
                acceptValue = "*/*";
                break;
            }
            default: {
                acceptValue = "*/*";
                break;
            }
        }
        headers.set("accept", acceptValue);
    }
    // TODO: Add accept-encoding header
    if (!headers.has("accept-language")) {
        headers.set("accept-language", "en-US,en;q=0.9");
    }
    // TODO: Add cache and pragma header defaults
    if (!headers.has("sec-ch-ua")) {
        headers.set("sec-ch-ua", '"Brave";v="129", "Not=A?Brand";v="8", "Chromium";v="129"');
    }
    if (!headers.has("sec-ch-ua-mobile")) {
        headers.set("sec-ch-ua-mobile", "?0");
    }
    if (!headers.has("sec-ch-ua-platform")) {
        headers.set("sec-ch-ua-platform", '"Linux"');
    }
    if (!headers.has("sec-fetch-dest")) {
        headers.set("sec-fetch-dest", destination);
    }
    if (!headers.has("sec-fetch-mode")) {
        if (mode === "navigate") {
            headers.set("sec-fetch-mode", "navigate");
        } else {
            headers.set("sec-fetch-mode", "no-cors");
        }
    }
    if (!headers.has("sec-fetch-site")) {
        if (mode === "navigate") {
            headers.set("sec-fetch-site", "none");
        } else {
            headers.set("sec-fetch-site", "same-origin");
        }
    }
    if (!headers.has("sec-fetch-user") && mode === "navigate") {
        headers.set("sec-fetch-user", "?1");
    }
    if (!headers.has("sec-gpc")) {
        headers.set("sec-gpc", "1");
    }
    if (!headers.has("upgrade-insecure-requests")) {
        headers.set("upgrade-insecure-requests", "1");
    }
    if (!headers.has("user-agent")) {
        headers.set("user-agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36");
    }

    if (!headers.get("origin") || headers.get("origin") === self.location.origin) {
        headers.set("origin", currentOrigin);
    }
    if (!headers.get("referer") || headers.get("referer") === self.location.origin) {
        headers.set("referer", currentOrigin);
    }
}

export function applyHeaderChanges(headers: Headers, url: URL, isRequest: boolean) {
    let headersConfig = manifest.headers?.find((conf) => conf.test(url, ""));
    if (!headersConfig) {
        return;
    }

    for (let removeHeader of headersConfig.remove_headers || []) {
        let direction = orDefault(removeHeader.direction, RequestDirection.BOTH);
        if (
            direction === RequestDirection.BOTH
            || (isRequest && direction === RequestDirection.REQUEST)
            || (!isRequest && direction === RequestDirection.RESPONSE)
        ) {
            headers.delete(removeHeader.name);
        }
    }

    for (let renameHeader of headersConfig.rename_headers || []) {
        let direction = orDefault(renameHeader.direction, RequestDirection.BOTH);
        if (
            direction === RequestDirection.BOTH
            || (isRequest && direction === RequestDirection.REQUEST)
            || (!isRequest && direction === RequestDirection.RESPONSE)
        ) {
            let value = headers.get(renameHeader.name);
            if (value) {
                headers.delete(renameHeader.name);
                headers.set(renameHeader.name, value);
            }
        }
    }

    for (let addHeader of headersConfig.add_headers || []) {
        let direction = orDefault(addHeader.direction, RequestDirection.BOTH);
        if (
            direction === RequestDirection.BOTH
            || (isRequest && direction === RequestDirection.REQUEST)
            || (!isRequest && direction === RequestDirection.RESPONSE)
        ) {
            if (orDefault(addHeader.append, false)) {
                headers.append(addHeader.name, addHeader.value);
            } else {
                headers.set(addHeader.name, addHeader.value);
            }
        }
    }
}
