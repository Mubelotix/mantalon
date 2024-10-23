console.log("Solve capcha")
this.placePixelElement.textContent = "Veuillez patienter...";
try {
    let resp = await fetch("https://insagenda.fr/queue-capcha");
    let body_text = await resp.text();
    this.placePixelElement.textContent = "Placer";
    await this.placePixel(body_text);
} catch {
    this.placePixelElement.textContent = "Erreur";
}
