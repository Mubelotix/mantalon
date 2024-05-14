console.log("insaplace login.js loaded")

// Add a checkbox to authorize friends to place pixels
let emailSelector = document.querySelector("main > div > form > div.flex");
let newDiv = document.createElement("div");
emailSelector.after(newDiv);
newDiv.outerHTML = `
<div class="my-2">
    <div class="flex items-center">
    <input type="checkbox" id="authorize-friends" name="authorize-friends" checked class="form-checkbox h-4 w-4 text-primary-600">
    <label for="authorize-friends" class="ml-2 text-sm text-gray-700">Permettre à mes amis insagenda de placer mes pixels</label>
    </div>
</div>
`;

// Add a notice at the top of the page
var state = true;
window.localStorage.setItem("authorize-friends", state);
document.getElementById("authorize-friends").addEventListener("click", function() {
    if (state) {
        state = false;
    } else {
        state = true;
    }
    window.localStorage.setItem("authorize-friends", state);
});

let form = document.querySelector("main > div > form");
let newDiv2 = document.createElement("div");
form.before(newDiv2);
newDiv2.outerHTML = `
<div class="bg-blue-100 border-t border-b border-blue-500 text-blue-700 px-4 py-3" role="alert">
  <p class="text-sm">Vous accédez à une version améliorée, non-officielle d'insaplace. Suggérez des fonctionnalités ou obtenez de l'aide à <a class="text-primary" target="_blank" href="https://mastodon.insa.lol/@insagenda">@insagenda@mastodon.insa.lol</a>.</p>
</div>
`;

// Remove the captcha
function waitForCaptchaAndRemove() {
    let captcha = document.querySelector("div[class^='g-recaptcha']");
    if (captcha !== null) {
        captcha.remove();
    } else {
        setTimeout(waitForCaptchaAndRemove, 100);
    }
}
waitForCaptchaAndRemove();

// Disable the submit button (make it non-submit)
let submit = document.querySelector("button[type='submit']");
submit.setAttribute("type", "button");

// Custom submit that solves the captcha
async function custom_submit() {
    try {
        submit.setAttribute("disabled", "disabled");
        submit.textContent = "Patientez 15 secondes la résolution du captcha...";
        let resp = await fetch("https://insagenda.fr/queue-capcha");
        let g_recaptcha_response = await resp.text();
        let captcha = document.createElement("input");
        captcha.setAttribute("type", "hidden");
        captcha.setAttribute("name", "g-recaptcha-response");
        captcha.setAttribute("value", g_recaptcha_response);
        form.appendChild(captcha);
        form.submit();
    } catch (e) {
        let message = {
            ty: "canPlace",
            data: false
        };
        window.parent.postMessage(message, "https://insagenda.fr/");
        window.parent.postMessage(message, "https://dev.insagenda.fr/");
        window.parent.postMessage(message, "http://localhost:8088/");        
    }
}
submit.addEventListener("click", custom_submit);

// Disable form submitting on enter
form.addEventListener("keypress", function(e) {
    if (e.key === "Enter") {
        e.preventDefault();
        custom_submit();
    }
});
