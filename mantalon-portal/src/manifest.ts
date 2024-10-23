
export class UrlMatcher {
    matches: URLPattern[];
    exclude_matches?: URLPattern[];
    
    // TODO: Support globs

    // TODO: Create something to match the initial loading page

    constructor(data: any) {
        let matches: string[] = data.matches;
        if (!Array.isArray(matches) || matches.some((pattern) => typeof pattern !== "string")) {
            throw new Error("UrlMatcher.matches must be an array of string");
        }
        this.matches = matches.map((pattern) => new URLPattern(pattern));
        
        let exclude_matches: string[] | undefined = data.exclude_matches;
        if (!exclude_matches) {
            return;
        }
        if (!Array.isArray(exclude_matches) || exclude_matches.some((pattern) => typeof pattern !== "string")) {
            throw new Error("UrlMatcher.exclude_matches must be an array of string");
        }
        this.exclude_matches = exclude_matches.map((pattern) => new URLPattern(pattern));
    }
}

enum RequestDirection {
    SERVER_BOUND = "server-bound",
    CLIENT_BOUND = "client-bound",
    BOTH = "both",
    REQUEST = CLIENT_BOUND,
    RESPONSE = SERVER_BOUND,
}

export class ProxyConfig extends UrlMatcher {
    /// Whether to rewrite location headers. Defaults to true.
    rewrite_location?: boolean;

    constructor(data: any) {
        super(data);

        if (data.rewrite_location && typeof data.rewrite_location !== "boolean") {
            throw new Error("ProxyConfig.rewrite_location must be a boolean");
        }
        this.rewrite_location = data.rewrite_location;
    }
}

export class ContentScriptsConfig extends UrlMatcher {
    /// An array of paths referencing CSS files that will be injected into matching pages.
    css?: string[];

    /// An array of paths referencing JavaScript files that will be injected into matching pages.
    js?: string[];

    constructor(data: any) {
        super(data);

        let css: string[] | undefined = data.css;
        if (css) {
            if (!Array.isArray(css) || css.some((path) => typeof path !== "string")) {
                throw new Error("ContentScriptsConfig.css must be an array of strings");
            }
            this.css = css;
        }

        let js: string[] | undefined = data.js;
        if (js) {
            if (!Array.isArray(js) || js.some((path) => typeof path !== "string")) {
                throw new Error("ContentScriptsConfig.js must be an array of strings");
            }
            this.js = js;
        }
    }
}

export class AddHeaderConfig {
    /// Whether it's a request or a response header
    direction: RequestDirection;

    /// The header to act on
    name: string;

    /// The value to set the header to
    value: string;

    /// Whether to set or append the header. Defaults to set.
    append?: boolean;

    constructor(data: any) {
        if (!Object.values(RequestDirection).includes(data.direction)) {
            throw new Error("AddHeaderConfig.direction must be one of RequestDirection");
        }
        this.direction = data.direction;

        if (typeof data.name !== "string") {
            throw new Error("AddHeaderConfig.name must be a string");
        }
        this.name = data.name;

        if (typeof data.value !== "string") {
            throw new Error("AddHeaderConfig.value must be a string");
        }
        this.value = data.value;

        if (data.append && typeof data.append !== "boolean") {
            throw new Error("AddHeaderConfig.append must be a boolean");
        }
        this.append = data.append;
    }
}

export class RemoveHeaderConfig {
    /// Whether it's a request or a response header
    direction: RequestDirection;

    /// The header to act on
    name: string;

    constructor(data: any) {
        if (!Object.values(RequestDirection).includes(data.direction)) {
            throw new Error("RemoveHeaderConfig.direction must be one of RequestDirection");
        }
        this.direction = data.direction;

        if (typeof data.name !== "string") {
            throw new Error("RemoveHeaderConfig.name must be a string");
        }
        this.name = data.name;
    }
}

export class RenameHeaderConfig {
    /// Whether it's a request or a response header
    direction: RequestDirection;

    /// The header to act on
    name: string;

    /// The new name of the header
    new_name: string;

    constructor(data: any) {
        if (!Object.values(RequestDirection).includes(data.direction)) {
            throw new Error("RenameHeaderConfig.direction must be one of RequestDirection");
        }
        this.direction = data.direction;

        if (typeof data.name !== "string") {
            throw new Error("RenameHeaderConfig.name must be a string");
        }
        this.name = data.name;

        if (typeof data.new_name !== "string") {
            throw new Error("RenameHeaderConfig.new_name must be a string");
        }
        this.new_name = data.new_name;
    }
}

export class HeadersConfig extends UrlMatcher {
    add_headers?: AddHeaderConfig[];
    remove_headers?: RemoveHeaderConfig[];
    rename_headers?: RenameHeaderConfig[];

    constructor(data: any) {
        super(data);

        let add_headers = data.add_headers;
        if (add_headers) {
            if (!Array.isArray(add_headers)) {
                throw new Error("HeadersConfig.add_headers must be an array");
            }
            this.add_headers = new Array();
            for (let header of add_headers) {
                this.add_headers.push(new AddHeaderConfig(header));
            }
        }

        let remove_headers = data.remove_headers;
        if (remove_headers) {
            if (!Array.isArray(remove_headers)) {
                throw new Error("HeadersConfig.remove_headers must be an array");
            }
            this.remove_headers = new Array();
            for (let header of remove_headers) {
                this.remove_headers.push(new RemoveHeaderConfig(header));
            }
        }

        let rename_headers = data.rename_headers;
        if (rename_headers) {
            if (!Array.isArray(rename_headers)) {
                throw new Error("HeadersConfig.rename_headers must be an array");
            }
            this.rename_headers = new Array();
            for (let header of rename_headers) {
                this.rename_headers.push(new RenameHeaderConfig(header));
            }
        }
    }
}

export class HandlerConfig extends UrlMatcher {
    /// Whether to run the script on the request or the response
    direction: RequestDirection;

    /// The path to the script to run.
    /// The file will be imported by the service worker, keep in mind that it doesn't run in a module context.
    /// You must define a function taking and returning a Request or Response object as an argument. 
    js: string;

    constructor(data: any) {
        super(data);

        if (!Object.values(RequestDirection).includes(data.direction)) {
            throw new Error("HandlerConfig.direction must be one of RequestDirection");
        }
        this.direction = data.direction;

        if (typeof data.js !== "string") {
            throw new Error("HandlerConfig.js must be a string");
        }
        this.js = data.js;
    }
}

export class RewriteConfig extends UrlMatcher {
    /// The URL to load instead of the original one
    destination: string;

    /// Whether to quietly rewrite the URL or redirect the request. Defaults to false.
    redirect?: boolean;

    constructor(data: any) {
        super(data);

        if (typeof data.destination !== "string") {
            throw new Error("RewriteConfig.destination must be a string");
        }
        this.destination = data.destination;

        if (data.redirect && typeof data.redirect !== "boolean") {
            throw new Error("RewriteConfig.redirect must be a boolean");
        }
        this.redirect = data.redirect
    }
}

export class Substitution {
    /// The regex pattern to match in the data
    pattern: string;

    /// The replacement string
    replacement: string;

    /// Whether to replace a single or all occurrences
    once?: boolean;

    // TODO: Support request body substitution

    constructor(data: any) {
        if (typeof data.pattern !== "string") {
            throw new Error("Substitution.pattern must be a string");
        }
        this.pattern = data.pattern;

        if (typeof data.replacement !== "string") {
            throw new Error("Substitution.replacement must be a string");
        }
        this.replacement = data.replacement;

        if (data.once && typeof data.once !== "boolean") {
            throw new Error("Substitution.once must be a boolean");
        }
        this.once = data.once;
    }
}

export class SubstitutionConfig extends UrlMatcher {
    substitutions: Substitution[];

    constructor(data: any) {
        super(data);

        let substitutions = data.substitutions;
        if (!Array.isArray(substitutions)) {
            throw new Error("SubstitutionConfig.substitutions must be an array");
        }
        this.substitutions = new Array();
        for (let substitution of substitutions) {
            this.substitutions.push(new Substitution(substitution));
        }
    }
}

export class Manifest {
    /// A list of websites that can be portaled to. The first one is the default and is required.
    targets: string[];

    /// The endpoint to connect to the Mantalon server
    server_endpoint: string;

    /// Instructs the portal to override URLs.
    /// If a URL matches any of these patterns, the portal will load the specified URL instead, without any detectable redirection.
    rewrites?: RewriteConfig[];

    /// Instructs the portal to edits headers on requests or responses.
    headers?: HeadersConfig[];

    /// Instructs the portal to run scripts to process requests or responses.
    handlers?: HandlerConfig[];

    /// Instructs the portal to proxy URLs. If an url matches any of these patterns, it will be proxied.
    /// Top-level pages are always proxied.
    /// Optional as by default, all urls are proxied.
    proxy_urls: ProxyConfig[];

    /// Instructs the portal to load content scripts into web pages whose URL matches a given pattern.
    content_scripts?: ContentScriptsConfig[];

    /// Instructs the portal to substitute data in the response body.
    /// If a URL matches any of these patterns, the portal will replace the specified pattern with the specified replacement.
    /// Warning: this disables body streaming, so it musn't be used on massive/media files.
    substitutions?: SubstitutionConfig[];

    // TODO: Add cache features

    constructor(data: any) {
        // Validate and set targets
        if (!Array.isArray(data.targets) || data.targets.length === 0 || data.targets.some((target: any) => typeof target !== "string")) {
            throw new Error("Manifest.targets must be a non-empty array of strings");
        }
        this.targets = data.targets;

        // Validate and set server_endpoint
        if (typeof data.server_endpoint !== "string") {
            throw new Error("Manifest.server_endpoint must be a string");
        }
        this.server_endpoint = data.server_endpoint;

        // Validate and set optional rewrites
        if (data.rewrites) {
            if (!Array.isArray(data.rewrites)) {
                throw new Error("Manifest.rewrites must be an array");
            }
            this.rewrites = data.rewrites.map(rewrite => new RewriteConfig(rewrite));
        }

        // Validate and set optional headers
        if (data.headers) {
            if (!Array.isArray(data.headers)) {
                throw new Error("Manifest.headers must be an array");
            }
            this.headers = data.headers.map(header => new HeadersConfig(header));
        }

        // Validate and set optional handlers
        if (data.handlers) {
            if (!Array.isArray(data.handlers)) {
                throw new Error("Manifest.handlers must be an array");
            }
            this.handlers = data.handlers.map(handler => new HandlerConfig(handler));
        }

        // Validate and set optional proxy_urls
        if (data.proxy_urls) {
            if (!Array.isArray(data.proxy_urls)) {
                throw new Error("Manifest.proxy_urls must be an array");
            }
            this.proxy_urls = data.proxy_urls.map(proxy => new ProxyConfig(proxy));
        }
        if (!this.proxy_urls || this.proxy_urls.length === 0) {
            this.proxy_urls = [new ProxyConfig({ matches: ["*://*"] })];
        }

        // Validate and set optional content_scripts
        if (data.content_scripts) {
            if (!Array.isArray(data.content_scripts)) {
                throw new Error("Manifest.content_scripts must be an array");
            }
            this.content_scripts = data.content_scripts.map(script => new ContentScriptsConfig(script));
        }

        // Validate and set optional substitutions
        if (data.substitutions) {
            if (!Array.isArray(data.substitutions)) {
                throw new Error("Manifest.substitutions must be an array");
            }
            this.substitutions = data.substitutions.map(sub => new SubstitutionConfig(sub));
        }
    }
}

async function loadManifestFromNetwork(): Promise<Manifest> {
    const response = await fetch("/mantalon/config/manifest.json");
    let cache = await caches.open("mantalon-sw-files");
    cache.put("/mantalon/config/manifest.json", response.clone());
    let data: any = await response.json();
    let manifest = new Manifest(data);
    return manifest;
}

export async function loadManifest(): Promise<Manifest> {
    try {
        return loadManifestFromNetwork();
    } catch {
        let cache = await caches.open("mantalon-sw-files");
        let request = await cache.match("/mantalon/config/manifest.json");
        if (!request) {
            throw new Error("Failed to load manifest");
        }
        let data: any = await request.json();
        let manifest = new Manifest(data);
        return manifest;
    }
}
