
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

    constructor(controller) {
        this._controller = controller;
        this.overlayCanvas = this._controller._createCanvas();
        this.overlayImageCanvas = this._controller._createCanvas();

        this._controller.boardDisplay.insertBefore(this.overlayCanvas, this._controller.selectionCanvasElement);
        this.overlayCanvasCxt = this.overlayCanvas.getContext('2d');

        this.overlay = new Image()
        this.overlay.crossOrigin = "Anonymous";
        this.overlay.src = "https://cdn.discordapp.com/attachments/878264604365574144/1239638340760895498/fotor-20240513195927.png?ex=6643a6ab&is=6642552b&hm=344bc5d70ab66563ec6fa3ab126b1d838902dbd2c232d7bf119ed1fe618e726d&"
        
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
                this._drawOverlay();
            } else {
                this.overlayCanvasCxt.clearRect(0, 0, this.overlayCanvas.width, this.overlayCanvas.height);
            }
        }
        
        document.querySelector(".border-t :last-child").appendChild(this.enableInput);
    }
 }

const masterController = new MasterController(controller);
console.log(masterController);
console.log("overlay.js loaded");
