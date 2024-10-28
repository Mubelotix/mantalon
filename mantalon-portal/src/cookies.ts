declare let self: ServiceWorkerGlobalScope;

import { Cookie } from "tough-cookie";
import { clientOrigins, cookieJar } from "./sw";

async function sendCookiesToClient(url: URL) {
    const matchingCookies = await cookieJar.getCookies(url.href);
    const cookieString = matchingCookies.map(cookie => `${cookie.key}=${cookie.value}`).join(';');
    self.clients.matchAll().then(clients => {
        for (let client of clients) {
            if (clientOrigins.get(client.id)?.startsWith(url.origin)) {
                client.postMessage({type: "mantalon-update-client-cookies", cookies: cookieString});
            }
        }
    });
}

async function updateCookieFromClient(url: URL, cookie: string) {
    let resCookie = Cookie.parse(cookie);
    if (!resCookie) {
        console.error("Failed to parse cookie from client");
        return;
    }

    resCookie = await cookieJar.setCookie(resCookie, url);
    if (!resCookie) {
        console.error("Failed to set cookie from client");
        return;
    }
    
    await sendCookiesToClient(url);
}
