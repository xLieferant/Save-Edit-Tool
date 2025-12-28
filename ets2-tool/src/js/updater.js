const updater = window.__TAURI__.updater;
const { relaunch } = window.__TAURI__.process;

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
            console.log("[updater] Kein Update verfügbar / Feed nicht erreichbar");
            showToast?.(
                "Update-Service derzeit nicht verfügbar. Prüfe Internetverbindung und Installation.",
                "warning"
            );
            return;
        }

        console.log(`[updater] Gefundenes Update: aktuelle Version ${update.currentVersion}, neue Version ${update.version}`);
        

        const shouldUpdate = confirm(
            `Update verfügbar: v${update.version}\n\nJetzt installieren?`
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
        console.error("[updater] Fehler beim manuellen Update-Check:", err);
        showToast?.("Update-Check fehlgeschlagen", "error");
    }
}
