
class MasterController {
    _controller = null;

    _drawOverlay() {
        this.overlayCanvasCxt.clearRect(0, 0, this.overlayCanvas.width, this.overlayCanvas.height);
        this.overlayCanvas.width = this._controller.board.width * 4;
        this.overlayCanvas.height = this._controller.board.height * 4;

        for(let x = 0; x < this._controller.board.width; x++) {
            for(let y = 0; y < this._controller.board.height; y++) {
                this.overlayCanvasCxt.fillStyle = "rgb(255, 0, 0)";
                this.overlayCanvasCxt.fillRect(x * 4 + 1, y * 4 + 1, 2, 2);
            }
        }

        var imageData = this._controller.boardCanvasCtx.getImageData(0, 0, this._controller.board.width, this._controller.board.height);
        console.log(imageData);
    }

    constructor(controller) {
        this._controller = controller;
        this.overlayCanvas = this._controller._createCanvas();
        this._controller.boardDisplay.insertBefore(this.overlayCanvas, this._controller.selectionCanvasElement);
        this.overlayCanvasCxt = this.overlayCanvas.getContext('2d');

        this.overlay = new Image()
        this.overlay.crossOrigin = "Anonymous";
        this.overlay.src = "http://localhost:8088/assets/screenshots/safari-screenshot.webp"
        
        this.overlay.onload = this._drawOverlay.bind(this);
        // Change opacity input
        this.opacityInput = document.createElement("input");
        this.opacityInput.type = "range";
        this.opacityInput.min = 0;
        this.opacityInput.max = 1;
        this.opacityInput.step = 0.01;
        this.opacityInput.value = 0.5;
        this.opacityInput.style.position = "relative";
        this.opacityInput.style.top = "0px";
        this.opacityInput.style.right = "0px";
        this.opacityInput.style.zIndex = 1000;
        
  
        this.opacityInput.oninput = () => {
            this._drawOverlay();    
        }
        
        document.querySelector(".border-t :last-child").appendChild(this.opacityInput);
    }
 }

const masterController = new MasterController(controller);
console.log(masterController);
console.log("overlay.js loaded");
