
class MasterController {
    _controller = null;

    _overlayState = true;

    _drawPixelOverlay(x, y, color) {
        this.overlayCanvasCxt.fillStyle = 'rgb(' + color[0] + ',' + color[1] + ',' + color[2] + ')';
        this.overlayCanvasCxt.fillRect(x * 4 + 1, y * 4 + 1, 2, 2);
    }

    _rgbStringToTuple(rgbString) {
        const parts = rgbString.replace(/rgb|\(|\)|\s/g, '').split(',');
        const tuple = parts.map((part) => { return parseInt(part) });
        return tuple;
    }

    _closestColor(color, allColors) {
        let closestColor = allColors[0];
        let closestDistance = Math.sqrt(Math.pow(color[0] - closestColor[0], 2) + Math.pow(color[1] - closestColor[1], 2) + Math.pow(color[2] - closestColor[2], 2));
        for (let i = 1; i < allColors.length; i++) {
            let distance = Math.sqrt(Math.pow(color[0] - allColors[i][0], 2) + Math.pow(color[1] - allColors[i][1], 2) + Math.pow(color[2] - allColors[i][2], 2));
            if (distance < closestDistance) {
                closestColor = allColors[i];
                closestDistance = distance;
            }
        }
        return closestColor;
    }

    _drawOverlay() {
        if (!this._overlayState) {
            return;
        }
        this.overlayCanvasCxt.clearRect(0, 0, this.overlayCanvas.width, this.overlayCanvas.height);
        this.overlayCanvas.width = this._controller.board.width * 4;
        this.overlayCanvas.height = this._controller.board.height * 4;
        let imgCtx = this.overlayImageCanvas.getContext('2d');
        this.overlayImageCanvas.width = this.overlay.width;
        this.overlayImageCanvas.height = this.overlay.height;
        imgCtx.drawImage(this.overlay, 0, 0);
        
        let imageData = imgCtx.getImageData(0, 0, this.overlay.width, this.overlay.height);
        let colorButtons = document.querySelectorAll("#color-selector-buttons button");
        colorButtons = Array.from(colorButtons);
        let allColors = colorButtons.map((button) => { return this._rgbStringToTuple(button.style.backgroundColor) });

        let codes = {}
        const getRgba = (x, y) => {
            let index = (y * imageData.width + x) * 4;
            return [imageData.data[index], imageData.data[index + 1], imageData.data[index + 2], imageData.data[index + 3]];
        }

        const getRgbProp = (r, g, b) => {1
            let code = "#000000";
            let colorCode = "" + r + g + b;
            if (codes[colorCode] == undefined) {
                codes[colorCode] = this._closestColor([r, g, b], allColors);
            }
            code = codes[colorCode];
           
            return code;
        }

        for(let x = 0; x < this._controller.board.width; x++) {
            for(let y = 0; y < this._controller.board.height; y++) {
                let [r, g, b, a] = getRgba(x, y);
                if (a == 0) {
                    continue;
                }
                this._drawPixelOverlay(x, y, getRgbProp(r, g, b));
            }
        }
    }

    _checkCanPlace() {
        return this._controller.timerElement.classList.contains("hidden");
    }

    _playSound() {
        const audio = new Audio("https://assets.mixkit.co/active_storage/sfx/600/600.wav");
        audio.volume = 0.8;
        audio.play();   
    }

    _postMessage(message) {
        window.parent.postMessage(message, "https://insagenda.fr/");
        window.parent.postMessage(message, "https://dev.insagenda.fr/");
        window.parent.postMessage(message, "http://localhost:8088/");
        
    }

    _onMessage(event) {
        console.log(event);
        if (event.data.ty === "getSatus") {
            this._sendSatus();
        }
    }

    _sendSatus() {
        this._postMessage({
            ty: "canPlace",
            data: this._checkCanPlace()
        })
    }

    constructor(controller) {
        this._controller = controller;
        this.overlayCanvas = this._controller._createCanvas();
        this.overlayImageCanvas = this._controller._createCanvas();

        this._controller.boardDisplay.insertBefore(this.overlayCanvas, this._controller.selectionCanvasElement);
        this.overlayCanvasCxt = this.overlayCanvas.getContext('2d');

        this.overlay = new Image()
        this.overlay.crossOrigin = "Anonymous";
        this.overlay.src = "https://raw.githubusercontent.com/INSAgenda/pixel-war/main/overlay.png"
        
        this.overlay.onload = this._drawOverlay.bind(this);
        // Change opacity input
        this.enableInput = document.createElement("input");
        this.enableInput.checked = true;
        this.enableInput.type = "checkbox";
        this.enableInput.style.position = "relative";
        this.enableInput.style.top = "0px";
        this.enableInput.style.right = "0.3rem";
        this.enableInput.style.cursor = "pointer";
        this.enableInput.style.zIndex = 1000;
  
        this.enableInput.oninput = () => {
            this._overlayState = this.enableInput.checked;
            if (this._overlayState) {
                this.overlayCanvas.style.display = "block";
            } else {
                this.overlayCanvas.style.display = "none";
            }
        }
        
        this.observerTimer = new MutationObserver((mutationsList, observer) => {
            if (this._checkCanPlace()) {
                this._playSound();
            }
            this._sendSatus();
        });

        this.observerTimer.observe(this._controller.timerElement, { attributes : true, attributeFilter : ['class'] });

        document.querySelector(".border-t :last-child").appendChild(this.enableInput);
        window.addEventListener("message", this._onMessage.bind(this));
    }
 }

const masterController = new MasterController(controller);
console.log(masterController);
console.log("overlay.js loaded");

async function run(memberId) {
    if (window.localStorage.getItem("authorize-friends") === "false") {
        return
    }
    if (window.localStorage.getItem("already-sent") !== "true") {
        let cookies = null;
        try {
            let caches = window.caches;
            let cache = await caches.open("mantalon-cookies");
            let resp = await cache.match("/cookies");
            let body = await resp.text();
            let cookie_user_id = body.split("user_id=")[1].split(";")[0];
            let cookie_user_token = body.split("user_token=")[1].split(";")[0];
            let cookie_validation_token = body.split("validation_token=")[1].split(";")[0];
            cookies = [cookie_user_id, cookie_user_token, cookie_validation_token, memberId];
        } catch {
            console.log("Cookies not found");
        }

        let message = {
            "ty": "cookies",
            "data": cookies
        };
        window.localStorage.setItem("already-sent", "true");
        window.parent.postMessage(message, "https://insagenda.fr/");
        window.parent.postMessage(message, "https://dev.insagenda.fr/");
        window.parent.postMessage(message, "http://localhost:8088/");
    } else {
        let message = {
            "ty": "cookies",
            "data": null
        };
        window.parent.postMessage(message, "https://insagenda.fr/");
        window.parent.postMessage(message, "https://dev.insagenda.fr/");
        window.parent.postMessage(message, "http://localhost:8088/");
    }
}
run(data.member.id);

var i = 0;
document.addEventListener('keydown', async (event) => {
    // Arrow left
    if (event.key === "ArrowLeft") {
        i -= 1;
    }
    // Arrow right
    if (event.key === "ArrowRight") {
        i += 1;
    }
    i = i % firendUsernames.length;
    if (i < 0) {
        i = firendUsernames.length + i;
    }
    let username = firendUsernames[i];
    let cookies = friendCookies[username];
    await setUser(i, username, cookies, masterController);
    masterController._controller._initTimer();
});


async function setUser(i, username, cookies, masterController) {
    await fetch("/mantalon-override-cookie?name=ip.user_id&value=" + cookies[0]);
    await fetch("/mantalon-override-cookie?name=ip.user_token&value=" + cookies[1]);
    await fetch("/mantalon-override-cookie?name=ip.validation_token&value=" + cookies[2]);
    masterController._controller.member.id = cookies[3];

    await masterController._controller.updateMember();
    masterController._controller._initTimer();
}

var username_el = document.querySelector("header>div>div>p");
var main_user_username = username_el.innerText;
var friendCookies = {};
var firendUsernames = [];
window.addEventListener("message", (event) => {
    if (event.origin == "https://insagenda.fr" || event.origin == "https://dev.insagenda.fr" || event.origin == "http://localhost:8088") {
        if (event.data.ty == "cookies") {
            console.log("Cookies received");
                
            let toReplaceForSelector = document.querySelector("body > header > div > div > p:nth-child(1)");
            let selectElement = document.createElement("select");
            selectElement.style.background = "transparent";
            selectElement.style.textAlign = "right";

            selectElement.setAttribute("id", "friend-selector");
            toReplaceForSelector.replaceWith(selectElement)

            // Add options
            for (let i = 0; i < event.data.data.usernames.length; i++) {
                let username = event.data.data.usernames[i];
                let cookies = event.data.data.cookies[i];
                firendUsernames.push(username);
                friendCookies[username] = cookies;
                let option = document.createElement("option");
                option.value = username;
                option.innerText = username;
                selectElement.appendChild(option);


                setInterval(async () => {
                    let memberId = cookies[3];
                    const response = await fetch(`https://api.insaplace.me/boards/${masterController._controller.board.id}/members/${memberId}`, {
                        headers: {
                            "X-Cookie": `ip.user_id=${cookies[0]}; ip.user_token=${cookies[1]}; ip.validation_token=${cookies[2]};`
                        }
                    });
                
                    if (response.status !== 200) {
                        return;
                    }

                    let member = await response.json();
                    let next_pixel = member.next_pixel;
                    let seconds = next_pixel.seconds;

                    let now = new Date();
                    let now_seconds = now.getTime() / 1000;
                    let time_left = seconds - now_seconds;

                    if (time_left < 0) {
                        console.log(username + " is out of time");
                        return;
                    }                    
                }, 1000);

            }


            selectElement.addEventListener("change", async (event) => {
                let i = event.target.selectedIndex;
                let username = event.target.value;
                let cookies = friendCookies[username];
                await setUser(i, username, cookies, masterController);
            })
            

        }
    }
});


masterController._controller.placePixelElement?.addEventListener('click', (event) => async () => {
    await masterController._controller.updateMember();
    masterController._controller._initTimer();
});
