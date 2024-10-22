# Mantalon

Mantalon empowers you to seamlessly inject code into third-party websites, adeptly circumventing CORS restrictions and CSP policies.

Moreover, Mantalon stands as a **privacy-preserving** solution, safeguarding the encryption between the client and the third-party website. Operating through TLS-encrypted HTTP streams, this proxy ensures data integrity unlike any other solution, such as nginx-proxied websites.

Notably, all code modifications and injections are executed solely on the client side, making it particularly well-suited for open-source projects.

## Project organization

This project is divided into three main components usable independently:
- `mantalon-server`: A server allowing opening TCP and UDP connections through websockets.
- `mantalon-client`: A client library providing a high-level API to interact with the server, notably a proxiedFetch function behaving like the native fetch function.
- `mantalon-portal`: A configurable service that you can use to create a live copy of any target website on your server, giving the ability to inject custom scripts, styles, and more, in a webextension-like fashion.

## Technical details

Three actors are involved in the Mantalon architecture:
- The client (end user)
- The proxy (you)
- The third-party target website

To utilize Mantalon, you must host the proxy on your own server. Clients establish a stream through the proxy to the target website. This stream is conveyed over WebSockets between the client and the proxy, and over TCP between the proxy and the target website. Before transmitting any HTTP request through the stream, clients initiate TLS encryption, ensuring that the content remains secure. Consequently, your proxy server could never see the content of the stream.

You furnish clients with instructions for modifying the target website. They can inject predefined scripts, manipulate headers, substitute text, redirect URLs, and more. Importantly, clients possess comprehensive knowledge of the modifications they apply. Users can verify these modifications without needing to place trust in the proxy.

The proxy has the capability to restrict service usage to a predefined list of allowed domains or IP addresses. As the proxy facilitates clients in establishing TCP streams to other internet peers, the applications extend far beyond just proxying HTTP websites. Possibilities include implementing other protocols like SSH, I2P, IPFS, BitTorrent, and more.

## Encryption security

Mantalon builds upon the foundational libraries of the Rust ecosystem, renowned for their unparalleled efficiency, reliability, and correctness. The encryption prowess of `ring` and `rustls` guarantees robust data protection, while the IO operations, powered by the trusted duo of `hyper` and `tokio`, ensure seamless and high-performance communication channels.

These libraries boast a track record of not just security but also efficiency, thanks to their carefully crafted design and rigorous testing. Their reliability is underscored by their widespread adoption in critical systems across industries. By harnessing these battle-tested dependencies, Mantalon embodies a commitment to delivering a solution that excels in both security and performance, earning the trust of users and developers alike.

## Usage (WIP)

```json
{
    "domains": ["en.wikipedia.org", "fr.wikipedia.org"],
    "landing_page": "landing.html",
    "lock_browsing": true,
    "https_only": true,
    "rewrite_location": true,
    "content_edits": [
        {
            "matches": ["https://*.wikipedia.org/wiki/Emacs", "https://*.wikipedia.org/wiki/Vim"],
            "override_url": "https://en.wikipedia.org/wiki/Shaitan"
        },
        {
            "matches": ["https://en.wikipedia.org/wiki/Amboise"],
            "js": "injected.js",
            "css": "style.css",
            "substitute": [
                {
                    "pattern": "Wikipedia",
                    "replacement": "Wikipedia (beta)",
                    "max_replacements": 1
                }
            ],
            "append_headers": {
                "Content-Security-Policy": "default-src 'self';"
            }
        },
        {
            "matches": ["https://*.wikipedia.org/wiki/*"],
            "js": "injected.js",
            "css": "pink-theme.css"
        }
    ]
}
```
