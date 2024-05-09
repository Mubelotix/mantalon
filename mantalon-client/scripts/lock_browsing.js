var pageHostname = window.location.hostname;
function controlledUrl(url) {
    if (!url) {
        return url;
    }
    try {
        let targetHostname = new URL(url).hostname;
        if (targetHostname != pageHostname && [proxiedDomains].includes(targetHostname)) { // TODO use origins
            let newUrl = new URL(url);
            newUrl.searchParams.set('mantalon-protocol', newUrl.protocol);
            newUrl.searchParams.set('mantalon-host', newUrl.host);
            newUrl.searchParams.set('mantalon-navigate', "true");
            newUrl.protocol = "http:";
            newUrl.host = "localhost:8000";
            return newUrl;
        } else {
            return url;
        }
    } catch (e) {
        return url;
    }
}

// Replace all links to external domains with a link to the proxy
document.addEventListener("click", function (e) {
    var target = e.target;
    if (target.tagName == 'A') {
        target.href = controlledUrl(target.href);
    }
}, true);

// Replace all iframe sources to external domains with a link to the proxy
function replaceIframeUrls() {
    let iframes = document.getElementsByTagName('iframe');
    for (let i = 0; i < iframes.length; i++) {
        let iframe = iframes[i];
        iframe.src = controlledUrl(iframe.src);
    }
}
replaceIframeUrls();

// Create a MutationObserver to watch for changes in the DOM
var observer = new MutationObserver(function(mutations) {
    mutations.forEach(function(mutation) {
        if (mutation.type === 'childList') {
            var newIframes = mutation.addedNodes;
            for (var i = 0; i < newIframes.length; i++) {
                if (newIframes[i].tagName === 'IFRAME') {
                    replaceIframeUrls();
                    break;
                }
            }
        }
    });
});
var observerConfig = { childList: true, subtree: true };
observer.observe(document.body, observerConfig);
