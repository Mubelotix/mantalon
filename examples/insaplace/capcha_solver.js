console.log("Solve capcha")
let resp = await fetch("https://insagenda.fr/queue-capcha");
let body_text = await resp.text();
await this.placePixel(body_text);
