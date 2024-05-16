let message = {
    ty: "canPlace",
    data: true
};
window.parent.postMessage(message, "https://insagenda.fr/");
window.parent.postMessage(message, "https://dev.insagenda.fr/");
window.parent.postMessage(message, "http://localhost:8088/");

window.addEventListener("message", async (event) => {
    if (event.origin == "https://insagenda.fr" || event.origin == "https://dev.insagenda.fr" || event.origin == "http://localhost:8088") {
        if (event.data.ty == "restoreCookies") {
            if (window.sessionStorage.getItem("already-restored") == "true") {
                return;
            } else {
                window.sessionStorage.setItem("already-restored", "true");
            }
            console.log("Cookies received");
            let cookies = event.data.data;
            await fetch("/mantalon-override-cookie?name=ip.user_id&value=" + cookies[0]);
            await fetch("/mantalon-override-cookie?name=ip.user_token&value=" + cookies[1]);
            await fetch("/mantalon-override-cookie?name=ip.validation_token&value=" + cookies[2]);
            window.localStorage.setItem("already-sent3", "true");
            console.log("Reload because cookies restored");
            window.location.reload();
        }
    }
});
