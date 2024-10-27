const handler = require("serve-handler");
const http = require("http");
const fs = require("fs");

const server = http.createServer((request, response) => {
    // Internal files
    let internal = false;
    if (request.url == "/mantalon/mantalon_client.js") {
        request.url = "/mantalon-client/pkg/mantalon_client.js";
        internal = true;
    } else if (request.url == "/mantalon/mantalon_client_bg.wasm") {
        request.url = "/mantalon-client/pkg/mantalon_client_bg.wasm";
        internal = true;
    } else if (request.url == "/mantalon/config/js-proxy-bundle.js") {
        request.url = "/mantalon-portal/js-proxy-bundle.js";
        internal = true;
    } else if (request.url == "/sw-bundle.js") {
        request.url = "/mantalon-portal/sw-bundle.js";
        internal = true;
    }
    if (internal) {
        return handler(request, response, {
            "public": "..",
        });
    }

    // Config files
    if (request.url.startsWith("/mantalon/config/")) {
        request.url = request.url.substring("/mantalon/config".length);
        return handler(request, response, {
            "public": "./examples/wikipedia",
        });
    }

    // Return index.html in all cases
    response.writeHead(200, { "Content-Type": "text/html" });
    fs.createReadStream("./index.html").pipe(response);
});

server.listen(3000, () => {
    console.log("Running at http://localhost:3000");
});
