let message = {
    ty: "canPlace",
    data: true
};
window.parent.postMessage(message, "https://insagenda.fr/");
window.parent.postMessage(message, "https://dev.insagenda.fr/");
window.parent.postMessage(message, "http://localhost:8088/");
