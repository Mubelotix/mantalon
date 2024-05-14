console.log("Solve capcha")
this.timerElement.textContent = "Veuillez patienter...";
try {
    let resp = await fetch("https://insagenda.fr/queue-capcha");
    let body_text = await resp.text();
    this.timerElement.textContent = "Fait!";
    await this.placePixel(body_text);
} catch {
    this.timerElement.textContent = "Erreur! Connectez-vous sur le site officiel";
}
