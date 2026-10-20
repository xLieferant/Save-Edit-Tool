# UI- und Lokalisierungsüberarbeitung

Stand: 20. Oktober 2026. Alle Änderungen liegen lokal vor. Es wurde nichts gepusht.

## Gestaltung

- Gemeinsames Designsystem in `src/styles/design_system.css`: neutrale dunkle Oberflächen, Segoe-UI-Schrifthierarchie, feste Abstandstokens, kleine Radien, dezente Rahmen und sichtbarer Tastaturfokus.
- Alte konkurrierende Theme-Tokens entfernt. Die vorhandenen Theme-Optionen bleiben verfügbar; neue Installationen verwenden den dunklen Standardstil.
- Dekorative Oberflächenverläufe, Glows, Textschatten und Hover-Bewegungen entfernt. Verläufe zur Daten- und Fortschrittsdarstellung sowie funktionale Ladeanimationen bleiben bestehen.
- Emojis aus Übersetzungstexten und HTML entfernt. Funktionale Spiel- und Navigationssymbole bleiben erhalten.
- Launcher, Editor, Karriere, Analytik, Mod-Profilverwaltung, Konfliktanalyse und Profilübertragung verwenden dieselben Oberflächen- und Bedienelementregeln.
- Die Werkzeugauswahl verwendet kompakte Aktionszeilen. Vorhandene Werkzeuge, Eventhandler und Editor-Reiter bleiben eingebunden.
- Profil- und Spielstandkennzahlen werden als zusammenhängender Informationsbereich dargestellt. Redundante Überschriften-Badges sind ausgeblendet.
- Glassmorphism bleibt auf hervorgehobenen Oberflächen erhalten: zurückhaltende Transparenz und 12-Pixel-Blur. Kontraste, Fokus, Hover und aktive Zustände bleiben sichtbar.
- Die zentrale `[hidden]`-Regel verhindert, dass Layoutregeln ausgeblendete Dialog-, Fehler- oder Ladebereiche sichtbar machen.

## Lokalisierung

Das bestehende System bleibt erhalten: JSON-Dateien in `src-tauri/locales`, der Rust-Translator und `translate_command`. Es gibt sieben Sprachen; fehlende Einträge anderer Sprachen verwenden weiterhin den englischen Fallback.

| Prüfung | Ergebnis |
|---|---:|
| Englische Schlüssel | 2.637 |
| Deutsche Schlüssel | 2.637 |
| Neu ergänzte deutsche Schlüssel | 251 |
| Davon zuvor nur englisch vorhandene Schlüssel | 69 |
| Neu ergänzte englische Schlüssel | 182 |
| Geänderte vorhandene deutsche Einträge | 388 |
| Davon aufgelöste `[NEEDS TRANSLATION]`-Einträge | 305 |
| Verbleibende deutsche Übersetzungsmarker | 0 |
| Fehler im automatisierten Schlüssel-/Platzhaltercheck | 0 |

Ergänzt beziehungsweise korrigiert wurden insbesondere LKW- und Aufliegerwechsel, Garagen, KI-Fahrer, Stammverzeichnisauswahl, Profilaktionen, Validierung, Einstellungen, Fortschrittsmeldungen, zugängliche Beschriftungen und native Dateidialoge. Auch 60 Backend-Diagnoseerklärungen und 15 angezeigte Diagnosekategorien sind angebunden.

Die eigenständigen Seiten verwenden dieselbe Dokumentübersetzung einschließlich `lang`, Platzhaltern, Tooltips und ARIA-Beschriftungen. Die Spracheinstellung des Karrieremodus aktualisiert jetzt auch den vorhandenen Translator und lädt die Oberfläche nach dem Speichern neu. Dynamische Meldungen verwenden Übersetzungsparameter. Unbekannte Fehler erhalten eine übersetzte Rückfallmeldung; technische Details bleiben im Protokoll.

Neue Einträge anderer Sprachen erhalten englische Fallbacktexte, keine ungeprüften fremdsprachigen Übersetzungen. Der Check meldet 5.787 Fallback-/Zusatzschlüsselhinweise für diese Sprachbestände und 218 Kandidaten ohne erkannte Quellreferenz. Dynamisch verwendete Schlüssel können darunter sein; es wurden keine Schlüssel gelöscht.

Technische IDs, Dateiformate, Profilnamen, Spielstanddaten und Originalzeilen aus SCS-Protokollen bleiben unverändert. Vollständige deutsche Darstellung aller möglichen Laufzeitdaten wird durch den statischen Check allein nicht bewiesen.

## Geänderte Dateien

### Design und Seiten

- `src/styles/design_system.css` (neu)
- `src/styles.css`
- `src/styles/launcher.css`
- `src/styles/career.css`
- `src/styles/analytics.css`
- `src/index.html`
- `src/pages/save-editor/index.html`
- `src/pages/career/index.html`
- `src/pages/analytics/index.html`
- `src/pages/mod-profile-manager/index.html`
- `src/pages/mod-conflict-analyzer/index.html`
- `src/pages/profile-sharing/index.html`

### Frontend-Lokalisierung

- `src/main.js`
- `src/app.js`
- `src/tools.js`
- `src/js/analytics.js`
- `src/modules/shared/i18n.js`
- `src/modules/career/main.js`

### Sprachdateien

- `src-tauri/locales/en.json`
- `src-tauri/locales/de.json`
- `src-tauri/locales/es.json`
- `src-tauri/locales/fr.json`
- `src-tauri/locales/it.json`
- `src-tauri/locales/pl.json`
- `src-tauri/locales/pt-BR.json`

### Backend-Texte

- `src-tauri/src/features/career/analytics.rs`
- `src-tauri/src/features/logging/service.rs`
- `src-tauri/src/features/mod_profile_manager/presets.rs`
- `src-tauri/src/features/profile_clone/commands.rs`
- `src-tauri/src/features/profile_sharing/service.rs`
- `src-tauri/src/features/save_analysis/export.rs`
- `src-tauri/src/features/save_analysis/service.rs`
- `src-tauri/src/features/settings/ets2_paths.rs`

### Prüfungen und Dokumentation

- `scripts/check_i18n.mjs` (neu)
- `scripts/check_ui.mjs` (neu)
- `UI_REVIEW.md` (dieser Bericht, neu)
- `../README.md`
- `../README.en.md`

Die bereits vor Beginn vorhandenen Änderungen an `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` und `src-tauri/tauri.conf.json` wurden nicht bearbeitet. Es wurden keine Abhängigkeiten ergänzt oder aktualisiert, keine Command-Signaturen geändert und keine Dateien gelöscht oder verschoben.

## Durchgeführte Prüfungen

| Prüfung | Ergebnis und Grenze |
|---|---|
| `node scripts/check_i18n.mjs` | Erfolgreich: deutsche Abdeckung, literale Quellreferenzen, Marker und Platzhalter |
| JavaScript-Syntax | 23 Frontend-Dateien beziehungsweise Inline-Module erfolgreich mit `node --check` geprüft |
| `node scripts/check_ui.mjs` | 42 Kombinationen aus sieben Seiten, zwei Sprachen und drei Fenstergrößen erfolgreich |
| Browserdetails | Keine unbehandelten Laufzeitfehler und kein horizontales Seitenüberlaufen; korrekte Sprachattribute, Sprachwechsel, Interpolation, versteckte Zustände, Stylesheet-Einbindung und mindestens 4,5:1 Textkontrast für geprüfte sichtbare aktive Buttons |
| Editor-Navigation | Alle fünf Reiter in beiden Sprachen mit Browser-Testdaten geöffnet |
| `cargo check --locked --offline` | Erfolgreich; vorhandene Compilerwarnungen bleiben bestehen |
| `cargo test --locked --offline` | 249 bestanden, 0 fehlgeschlagen, 1 ignoriert |
| `cargo build --locked --offline` | Erfolgreich |
| Nativer App-Start | Launcher und Save Editor im echten Tauri-WebView geladen; fünf Editor-Reiter vorhanden, keine sichtbaren fehlenden Schlüssel, keine unbehandelten JavaScript-Fehler beim geprüften Start |
| Tauri-Build | Durch Paketversionsprüfung blockiert, siehe unten |

Die Browserprüfung verwendet ausdrücklich Tauri-Testdaten. Sie prüft keine nativen Dateidialoge und keine echten Savegame-Schreibvorgänge. Die Rust-Tests enthalten unter anderem vorhandene Prüfungen für LKW-Besitzauflösung, Garagenänderungen und KI-Fahrerverwaltung. Sie ersetzen keinen vollständigen manuellen Desktop-Durchlauf.

Die Browser-Screenshots des abschließenden Prüflaufs liegen unter `%TEMP%\ets2-ui-review-mWfwdM`. Ein erneuter Prüflauf gibt seinen neuen Screenshotpfad aus. Die Prüfskripte benötigen keine neuen Pakete. `CHROME_PATH` kann einen anderen installierten Chromium-Browser angeben.

Eigenständiges Type Checking, Linting und ein Frontend-Build sind im Projekt nicht konfiguriert. Die Frontend-Dateien werden direkt ausgeliefert.

## Bekannte Einschränkungen und manuelle Nachprüfung

1. `cargo tauri build --debug --no-bundle --ci -- --locked --offline` stoppt bei den bereits vorhandenen Paketständen: Rust-Updater 2.10.1 gegenüber JavaScript-Updater 2.9.0 sowie Rust-Dateisystemplugin 2.5.1 gegenüber JavaScript-Dateisystemplugin 2.4.4. Die Versionsprüfung wurde nicht umgangen. Ein Installer wurde nicht erzeugt.
2. Beim nativen Start meldet die nicht geänderte Karriere-Datenbankinitialisierung einen SQL-Fehler `near "DO": syntax error`. Launcher und Editor laden trotzdem. Karriere-Datenabläufe können dadurch eingeschränkt sein.
3. Die vorhandene öffentliche Build-Sperre für Karrieremodus/Anmeldung bleibt erhalten. Sie wurde nicht aufgehoben.
4. `cargo tauri dev` wurde nicht als erfolgreicher Entwicklungsstart verifiziert. Der dokumentierte native Start erfolgt mit dem direkt gebauten Debug-Programm.
5. Nicht vollständig manuell ausgeführt wurden Profil-/Spielstandauswahl mit realen Daten, Backup und Wiederherstellung, LKW-/Aufliegerwechsel, KI-Fahrermutationen, Garagenänderungen, Map Unlocker, native Dateidialoge und alle Randfälle dynamischer Backendmeldungen. Bitte diese Abläufe mit einer Spielstandkopie in Deutsch und Englisch prüfen. Für diese Abläufe wird kein erfolgreicher manueller Test behauptet.

## Prüfkommandos

Aus `ets2-tool/`:

```powershell
node scripts/check_i18n.mjs
node scripts/check_i18n.mjs --json
node scripts/check_ui.mjs
```

Aus `ets2-tool/src-tauri/`:

```powershell
cargo check --locked --offline
cargo test --locked --offline
cargo build --locked --offline
```

Zur lokalen Sichtprüfung kann die direkt gebaute Datei `src-tauri/target/debug/ets2-tool.exe` gestartet werden.
