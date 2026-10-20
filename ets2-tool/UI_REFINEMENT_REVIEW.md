# UI-Verfeinerung: Themes, Bedienelemente und Arbeitsbereiche

Stand: 20.10.2026. Alle Änderungen liegen lokal; kein Commit oder Push.

## Änderungen

- **Themes:** Dark und Light sind die beiden Oberflächenthemes. Der optionale grüne Akzent wird separat gespeichert. Zentrale Farb-Tokens ersetzen feste Komponentenfarben für Flächen, Texte, Rahmen und Zustände. Die bestehende Typografie, Navigation, Button-Struktur und dezente Glaswirkung bleiben erhalten.
- **Migration:** `neon` wird ohne bestehende Akzentwahl zu Dark mit Grün, `neon-red` zu Dark mit Standardakzent. Ungültige Werte werden auf Dark/Standard gesetzt. Ein synchrones Head-Skript initialisiert alle sieben Seiten vor dem Stylesheet-Paint. Bei einem Speicherfehler wird keine erfolgreich übernommene Darstellung vorgetäuscht.
- **Switches:** Gemeinsame Anbindung an die native Checkbox: OFF links/rot, ON rechts/grün, ergänzender übersetzter Zustandstext und `aria-checked`. Auch der String `"0"` wird korrekt als ausgeschaltet behandelt. Maus, Leertaste, Fokus, Disabled und Loading bleiben bedienbar beziehungsweise erkennbar. Dynamische Zustandsbeschriftungen berücksichtigen Sprachwechsel.
- **Checkboxen:** Gemeinsame Größe, neutral-roter Rand im ausgeschalteten Zustand und grüner Hintergrund mit Häkchen im eingeschalteten Zustand. Der Zwischenzustand bleibt separat gekennzeichnet. Fehler- und Warnungsfarben werden nicht zu allgemeinen Aktivierungsfarben umgedeutet.
- **Einstellungen:** Hintergrundaktualisierungen überschreiben keine ungespeicherten Formulareingaben. Speicheraktionen sperren ihre Bedienelemente während der Anfrage; Fehler melden sich per Toast und stellen die bestätigten Werte wieder dar. Vorhandene Backend-Aufrufe bleiben bestehen.
- **Container:** Der Editor verwendet einen vertikalen Daten-Scrollbereich. Äußere dekorative Rahmen und zusätzliche Hintergründe um Werkzeug- und Garagenbereiche entfallen; die Garagenliste wächst mit ihren Einträgen. Keine Virtualisierung oder Änderung der Garagenfunktionen.
- **Sidebar:** Abschnitte schrumpfen nicht mehr über ihre Inhalte zusammen. Dadurch bleibt „Profile scannen“ vollständig sichtbar; längere Sidebar-Inhalte scrollen gemeinsam. `Created by xLieferant` steht klein unterhalb dieses Scrollbereichs und bleibt sichtbar.
- **Arbeitsbereich:** Eine gemeinsame Frontend-Anbindung verwendet die bestehenden `hub_get_mode`-/`hub_set_mode`-Commands. Ein explizit gespeicherter, unterstützter und mit dem Backend übereinstimmender Modus wird direkt geöffnet. Erststart, ungültiger Zustand und bewusstes Wechseln führen zur zentralen Auswahl. Der eingebettete zweite Hub bleibt im eigenständigen Editor verborgen. Die bestehende öffentliche Karriere-Sperre bleibt erhalten.
- **Lokalisierung:** Sieben neue Schlüssel in Englisch und Deutsch, darunter Akzentwahl, Speicherbestätigung, Workspace-Fehler und ungespeicherte Änderungen. Die weiteren Sprachdateien erhalten englische Fallback-Texte. Der ausdrücklich gewünschte Creator-Text bleibt unverändert.

Keine Rust-Quelldateien, Tauri-Command-Signaturen, Abhängigkeiten oder Savegame-Formate wurden in diesem Arbeitsschritt verändert.

## Geänderte Dateien

- Neu: `src/modules/shared/theme.js`, `theme_settings.js`, `controls.js`, `settings_form.js`, `workspace.js`.
- Frontend-Logik: `src/app.js`, `main.js`, `tools.js`, `modules/launcher/main.js`, `modules/save-editor/standalone.js`, `modules/career/main.js`.
- HTML: `src/index.html` sowie `src/pages/{save-editor,career,analytics,mod-conflict-analyzer,mod-profile-manager,profile-sharing}/index.html`.
- CSS: `src/styles.css`, `src/styles/{design_system,career,launcher,analytics}.css`.
- Übersetzungen: `src-tauri/locales/{en,de,es,fr,it,pl,pt-BR}.json`.
- Prüfung/Dokumentation: `scripts/check_ui.mjs`, dieser Bericht, `../README.md`, `../README.en.md`.

## Tatsächlich ausgeführte Prüfungen

| Prüfung | Ergebnis |
|---|---|
| `node scripts/check_i18n.mjs` | 2644 EN-/DE-Schlüssel, 0 Fehler; 5787 Fallback-/Zusatzschlüsselhinweise und 221 möglicherweise dynamisch verwendete Einträge ohne erkannte Referenz |
| JavaScript-Syntax | 28 Dateien beziehungsweise Inline-Skripte mit `node --check` geprüft, 0 Fehler |
| `node scripts/check_ui.mjs` | 42 Seiten-/Sprach-/Fensterprüfungen und alle fünf Editor-Reiter in Deutsch und Englisch erfolgreich |
| Theme-/Akzentmatrix | 56 Prüfungen: sieben Seiten, zwei Sprachen, Dark/Light jeweils mit Standard/Grün; Farben, Persistenz und Textkontrast aktiver Buttons geprüft |
| Garagenmatrix | 96 Prüfungen: 0/1/10/50/100/125 Einträge, 900 × 640 und 1210 × 800, beide Sprachen und alle vier Erscheinungsbilder |
| Garagen-/Sidebar-Details | Einträge vollständig gerendert, letzte Garage erreichbar, höchstens ein aktiver vertikaler Daten-Scrollbereich, keine horizontal abgeschnittenen Garagen-Bedienelemente, kein abgeschnittener Scan-Button, letzte Sidebar-Aktion erreichbar und Credit im Fenster |
| Gezielt nach letzter Switch-Anpassung | `node scripts/check_ui.mjs --refinement-only`: 96 Garagenfälle sowie Maus-/Tastatur-/Disabled-Zustände, dynamische Sprache, Speichern mit Fehler-Rollback, Theme-Migration und Workspace-Auswahl erneut erfolgreich |
| `cargo check --locked --offline` | Erfolgreich, bestehende Compilerwarnungen |
| `cargo test --locked --offline` | 17 Bibliotheks- und 249 Anwendungstests bestanden; 0 fehlgeschlagen, 1 ignoriert |
| `cargo build --locked --offline` | Erfolgreich, bestehende Compilerwarnungen |
| Echter Tauri-WebView | Deutsch, Theme-Bootstrap, fünf Reiter und Credit geprüft; Light/Grün nach Neuladen erhalten und ursprüngliche Darstellung wiederhergestellt; gespeicherten Editor direkt geöffnet und über „Hub öffnen“ bewusst erneut ausgewählt; keine zweite Auswahl, keine unbehandelten JavaScript-Ausnahmen im Prüfzeitraum |
| `git diff --check` | Erfolgreich unter Berücksichtigung der vorhandenen CRLF-Konvention |

Der vollständige Browserlauf erzeugte Screenshots unter `%TEMP%\ets2-ui-review-wyOJ67`. Der anschließende gezielte Lauf liegt unter `%TEMP%\ets2-ui-review-6gtOXe`. Erneute Aufrufe geben ihren eigenen Artefaktpfad aus. Die Browserprüfungen verwenden isolierte Tauri-Testdaten; sie verändern keine echten Spielstände. Die native Startprüfung verwendet die direkt gebaute Debug-Anwendung und schreibt ebenfalls keine Spielstandänderungen.

## Grenzen

- `cargo tauri build --debug --no-bundle --ci -- --locked --offline` bleibt durch die bestehenden Versionen blockiert: Rust-Updater 2.10.1 gegenüber JS-Updater 2.9.0 und Rust-Filesystem 2.5.1 gegenüber JS-Filesystem 2.4.4. Kein Installer erzeugt, keine Versionsprüfung umgangen.
- Die bestehende Karriere-Datenbankinitialisierung meldet beim nativen Start weiterhin `near "DO": syntax error`. Karriere-Datenabläufe sind dadurch nicht als vollständig funktionierend verifiziert.
- Eigenständiges Type Checking, Linting und ein Frontend-Build sind nicht eingerichtet; die Anwendung liefert HTML/CSS/JavaScript direkt aus. `cargo tauri dev` wurde nicht als erfolgreicher Entwicklungsstart verifiziert.
- Native Dateidialoge, alle realen Profil-/Savegame-Randfälle und schreibende Abläufe für Backups, LKW, Auflieger, KI-Fahrer, Garagen oder Kartenfreischaltung wurden nicht vollständig manuell ausgeführt. Diese Abläufe bitte mit Spielstandkopien nachprüfen. Aus den Browser-Testdaten wird kein erfolgreicher realer Savegame-Test abgeleitet.
