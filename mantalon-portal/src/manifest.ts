
export interface UrlMatcher {
    matches: string[];
    exclude_matches?: string[];
    
    // TODO: Support globs

    // TODO: Create something to match the initial loading page
}

enum RequestDirection {
    SERVER_BOUND,
    CLIENT_BOUND,
    BOTH,
    REQUEST = CLIENT_BOUND,
    RESPONSE = SERVER_BOUND,
}

export interface ProxyConfig extends UrlMatcher {
    /// The endpoint to proxy the request to. Defaults to the global value
    server_endpoint?: string;

    /// Whether to rewrite location headers
    rewrite_location?: boolean;
}

export interface ContentScriptsConfig extends UrlMatcher {
    /// An array of paths referencing CSS files that will be injected into matching pages.
    css?: string[];

    /// An array of paths referencing JavaScript files that will be injected into matching pages.
    js?: string[];
}

export interface AddHeaderConfig {
    /// Whether it's a request or a response header
    direction: RequestDirection;

    /// The header to act on
    name: string;

    /// The value to set the header to
    value: string;

    /// Whether to set or append the header. Defaults to set.
    append?: boolean;
}

export interface RemoveHeaderConfig {
    /// Whether it's a request or a response header
    direction: RequestDirection;

    /// The header to act on
    name: string;
}

export interface RenameHeaderConfig {
    /// Whether it's a request or a response header
    direction: RequestDirection;

    /// The header to act on
    name: string;

    /// The new name of the header
    new_name: string;
}

export interface HeadersConfig extends UrlMatcher {
    add_headers: AddHeaderConfig[];
    remove_headers: RemoveHeaderConfig[];
    rename_headers: RenameHeaderConfig[];
}

export interface HandlerConfig extends UrlMatcher {
    /// Whether to run the script on the request or the response
    direction: RequestDirection;

    /// The path to the script to run.
    /// The file will be imported by the service worker, keep in mind that it doesn't run in a module context.
    /// You must define a function taking and returning a Request or Response object as an argument. 
    js: string;
}

export interface RewriteConfig extends UrlMatcher {
    /// The URL to load instead of the original one
    destination: string;
}

export interface Substitution {
    /// The regex pattern to match in the data
    pattern: string;

    /// The replacement string
    replacement: string;

    /// Whether to replace a single or all occurrences
    once?: boolean;

    // TODO: Support request body substitution
}

export interface SubstitutionConfig extends UrlMatcher {
    substitutions: Substitution[];
}

export interface Manifest {
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
    /// By default, all urls are proxied.
    proxy_urls?: ProxyConfig[];

    /// Instructs the portal to load content scripts into web pages whose URL matches a given pattern.
    content_scripts?: ContentScriptsConfig[];

    /// Instructs the portal to substitute data in the response body.
    /// If a URL matches any of these patterns, the portal will replace the specified pattern with the specified replacement.
    /// Warning: this disables body streaming, so it musn't be used on massive/media files.
    substitutions?: SubstitutionConfig[];

    // TODO: Add cache features
}

async function loadManifestFromNetwork(): Promise<Manifest> {
    const response = await fetch("/mantalon/config/manifest.json");
    let cache = await caches.open("mantalon-sw-files");
    cache.put("/mantalon/config/manifest.json", response);
    return response.json();
}

export async function loadManifest(): Promise<Manifest> {
    try {
        return loadManifestFromNetwork();
    } catch {
        let cache = await caches.open("mantalon-sw-files");
        let request = await cache.match("/mantalon/config/manifest.json");
        return request?.json(); // Fixme: Investigate why we don't have to add undefined to the signature?
    }
}
