# Mantalon

Mantalon enables you to inject code into iframes of third-party websites. Mantalon will help you breaking CORS restrictions and CSP policies.

All of this is **done without voiding the encryption** between the client and the third-party website. This is the killer feature compared to other solutions, like proxying the third-party website using nginx. 

With Mantalon, all code modifications are done on the client side. This is perfect for open-source projects.

## How does it work?

Mantalon uses a proxy to evade CORS restrictions, but proxies the encrypted stream rather than raw HTTP requests. A service worker is plugged into the target website to force requests going through the proxy.

In order to use Mantalon, you need to host the proxy on your own server. The proxy supports whitelisting over both source and target domains to prevent abuse.

## Encryption security

Mantalon relies on the cornerstone libraries of the Rust ecosystem. Encryption is done by `ring` and `rustls` while IO is made with `hyper` and `tokio`. These are extremely secure and battle-tested dependencies.

## Usage


