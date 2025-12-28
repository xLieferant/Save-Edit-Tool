const updater = window.__TAURI__.updater;
const { relaunch } = window.__TAURI__.process;
const { openUrl } = window.__TAURI__.opener;

/**
 * Prüft beim Start, ob ein Update verfügbar ist und installiert es ggf.
 */
export async function checkUpdaterOnStartup(showToast) {
    try {
        const update = await updater.check();

        console.log("[updater] Update-Objekt:", update);
        console.log("[updater] Current Version:", update.currentVersion);
        console.log("[updater] Latest Version:", update.version);
        console.log("[updater] Should Update:", update.shouldUpdate);

        if (!update || update.shouldUpdate === false) {
            showToast?.(
                "Update-Service derzeit nicht verfügbar oder keine neue Version gefunden.",
                "warning"
            );
            return;
        }

        const shouldUpdate = confirm(
            `Ein Update auf Version ${update.version} ist verfügbar.\n\nJetzt herunterladen und installieren?`
        );
        if (!shouldUpdate) return;

        showToast?.("Update wird heruntergeladen …", "info");

        await update.downloadAndInstall((event) => {
            switch (event.event) {
                case "Started":
                    console.log(`[Updater] Download gestartet: ${event.data.contentLength ?? 0} bytes`);
                    break;
                case "Progress":
                    console.log(`[Updater] Fortschritt: ${event.data.chunkLength ?? 0} bytes`);
                    break;
                case "Finished":
                    console.log("[Updater] Download abgeschlossen");
                    break;
            }
        });

        showToast?.("Update installiert – Neustart …", "success");
        await relaunch();

    } catch (err) {
        console.error("[updater] Fehler beim Auto-Update:", err);
        showToast?.("Auto-Update fehlgeschlagen", "error");
    }
}

/**
 * Manuelles Update-Check
 */
export async function manualUpdateCheck(showToast) {
    try {
        showToast?.("Suche nach Updates …", "info");

        const update = await updater.check();

        if (!update) {
            console.warn("[updater] Update-Feed nicht erreichbar");
            showToast?.(
                "Update-Service derzeit nicht verfügbar. Prüfe Internetverbindung und Installation.",
                "warning"
            );
            return;
        }

        if (update.version === update.currentVersion) {
            console.log("[updater] Keine Updates verfügbar");
            showToast?.("Keine Updates verfügbar. Du hast die aktuelle Version.", "success");
            return;
        }

        console.log(`[updater] Gefundenes Update: aktuelle Version ${update.currentVersion}, neue Version ${update.version}`);
        showToast?.(`Neue Version verfügbar: ${update.version}`, "info");

        // ---- Neuer Teil: GitHub-Link öffnen ----
        const shouldOpen = confirm(
            `Update verfügbar: v${update.version}\n\nMöchtest du die neue Version auf GitHub herunterladen?`
        );

        if (shouldOpen) {
            // GitHub-Release-Seite öffnen (hier musst du ggf. deine URL anpassen)
            await openUrl("https://github.com/xLieferant/Save-Edit-Tool/releases/latest");
        }

    } catch (err) {
        console.error("[updater] Fehler beim manuellen Update-Check:", err);
        showToast?.(
            "Update-Service derzeit nicht verfügbar oder Update fehlgeschlagen.",
            "warning"
        );
    }
}
