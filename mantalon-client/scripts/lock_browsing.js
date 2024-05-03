var pageHostname = window.location.hostname;
document.addEventListener("click", function (e) {
    var target = e.target;
    if (target.tagName == 'A') {
        let targetHostname = new URL(target.href).hostname;
        if (targetHostname != pageHostname && [proxiedDomains].includes(targetHostname)) { // TODO use origins
            target.href = "/mantalon/navigate?url=" + encodeURIComponent(target.href);
        }
    }
}, true);
