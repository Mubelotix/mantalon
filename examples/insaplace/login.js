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

// When checkbox is clicked
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
