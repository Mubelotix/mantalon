import { loadRessource } from "./manifest";
import { cookieJar, manifest, orDefault } from "./sw";

const recast = require("recast");
const parse5 = require("parse5");

const JAVASCRIPT_MIME_TYPES = new Set([
    "module", // for <script type=module>
    "text/javascript",
    "application/javascript",
    "application/ecmascript",
    "application/x-ecmascript",
    "application/x-javascript",
    "text/ecmascript",
    "text/javascript1.0",
    "text/javascript1.1",
    "text/javascript1.2",
    "text/javascript1.3",
    "text/javascript1.4",
    "text/javascript1.5",
    "text/jscript",
    "text/livescript",
    "text/x-ecmascript",
    "text/x-javascript",
]);

function applyJsProxyOnJs(input: string): string {
    const ast = recast.parse(input);
    recast.types.visit(ast, {
        visitIdentifier(path) {
            if (path.node.name === "window") {
                path.replace(recast.types.builders.identifier("proxiedWindow"));
                return false;
            } else if (path.node.name === "document") {
                path.replace(recast.types.builders.identifier("proxiedDocument"));
                return false;
            } else if (path.node.name === "location") {
                path.replace(recast.types.builders.identifier("proxiedLocation"));
                return false;
            }

            this.traverse(path);
        },
    });
    
    return recast.print(ast).code;
}

function applyJsProxyOnDoc(input) {
    // Parse the document with parse5
    let document = parse5.parse(input);

    // Helper function to recursively find and modify script tags
    function traverseAndModify(node) {
        if (node.tagName === "script" && node.attrs) {
            // Get the `type` attribute from the script tag if it exists
            const typeAttr = node.attrs.find(attr => attr.name === "type");
            const typeValue = typeAttr ? typeAttr.value : "text/javascript";

            // Only modify scripts with allowed types
            if (JAVASCRIPT_MIME_TYPES.has(typeValue) && node.childNodes && node.childNodes.length > 0) {
                // Assume textContent is in the first child node of the script tag
                let scriptTextNode = node.childNodes[0];
                let input = scriptTextNode.value;
                try {
                    let output = applyJsProxyOnJs(input);
                    scriptTextNode.value = output;
                } catch(e) {
                    console.error("Failed to apply JS proxy on script tag", e);
                }
            }
        }

        // Recursively traverse child nodes if they exist
        if (node.childNodes) {
            for (let childNode of node.childNodes) {
                traverseAndModify(childNode);
            }
        }
    }

    // Start traversal on the document's root node
    traverseAndModify(document);

    // Serialize the modified document back to HTML
    let outputHtml = parse5.serialize(document);
    return outputHtml;
}

export async function applyJsProxy(response: Response, url: URL, contentType: string, clientId: string): Promise<string | undefined> {
    if (!response.body) {
        return undefined;
    }
    
    const jsProxyConfig = manifest.js_proxies?.find((conf) => conf.test(url, contentType));
    if (!jsProxyConfig) {
        return undefined;
    }

    if (!orDefault(jsProxyConfig.enabled, false)) {
        return undefined;
    }

    // TODO: Add more content types
    let bodyText;
    try {
        if (contentType.includes("text/html")) {
            bodyText = await response.text();
            let html = applyJsProxyOnDoc(bodyText);
            let bundleResponse = await loadRessource("js-proxy-bundle.js");
            if (!bundleResponse) {
                console.error("Failed to load JS proxy bundle");
                return undefined;
            }
    
            let content = await bundleResponse.text();
            const matchingCookies = await cookieJar.getCookies(url); // TODO: Support http-only cookie attributes
            const cookieString = matchingCookies.map(cookie => `${cookie.key}=${cookie.value}`).join(';');
            content = content.replace(`"init-origin"`, `"${url.origin}"`);
            content = content.replace(`"init-cookies"`, `"${cookieString}"`);
            content = content.replace(`"init-clientId"`, `"${clientId}"`);
            content = content.replace(`new Set(["init-targetOrigins"])`, `new Set(${JSON.stringify(manifest.targets)})`);
            if (!html.includes(content)) {
                if (!html.includes("<head>")) {
                    console.error("Failed to inject JS proxy bundle: <head> not found in document");
                    return undefined;
                }
                html = html.replace("<head>", `<head><script>${content}</script>`);
            }
            
            return html;
        } else if (contentType.includes("text/javascript")) {
            bodyText = await response.text();
            return applyJsProxyOnJs(bodyText);
        }
    } catch(e) {
        console.error(`Failed to apply JS proxy on ${url.href}: ${e}`);
        return bodyText
    }
    
    return undefined;
}
