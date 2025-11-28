# SCS Save Editor  
Ein moderner, plattformübergreifender Savegame-Editor für **Euro Truck Simulator 2**, entwickelt mit **Tauri**, **Rust** und **JavaScript**.

Der Editor ermöglicht das Auslesen, Bearbeiten und Schreiben von Save-Daten wie Geld, Level/XP und weiteren fahrerbezogenen Parametern. Ziel ist eine stabile, einfach bedienbare und erweiterbare Software für Windows, macOS und Linux.

---

## 🚀 Funktionen

- Automatische Suche nach ETS2-Profilen  
- Laden von `autosave` und relevanten `.sii`-Dateien  
- Anzeigen aktueller Spielwerte  
  - Geld  
  - Erfahrungspunkte (XP) / Level  
- Bearbeiten und Speichern von:
  - Geld  
  - Level / XP  
- Robuste Fehlerbehandlung  
- Moderne UI mit Tauri + Vanilla JavaScript  

> Weitere Werte wie Garagen, LKW-Daten, Städte, Fahrer usw. folgen später.

---

## 📁 Projektstruktur

projekt-root/
│
├─ src-tauri/
│ ├─ src/
│ │ ├─ commands.rs // Rust-Kommandos (find profiles, read, write)
│ │ ├─ helpers.rs // Parser & Utilities
│ │ └─ main.rs // Tauri-Konfiguration
│ └─ tauri.conf.json
│
├─ frontend/
│ ├─ index.html
│ ├─ main.js
│ ├─ styles.css
│
├─ README.md
└─ .gitignore

yaml
Code kopieren

---

## 🔧 Installation & Setup

### Anforderungen
- Rust (stable)
- Node.js & npm
- Tauri CLI  
  ```bash
  cargo install tauri-cli
Projekt starten
bash
Code kopieren
npm install
npm run tauri dev
Build für Release
bash
Code kopieren
npm run tauri build
Das Build-Artefakt befindet sich danach unter:

arduino
Code kopieren
src-tauri/target/release/
🧩 Funktionsweise (Kurz erklärt)
1. Profile erkennen
Rust scannt den Pfad:

bash
Code kopieren
Dokumente/Euro Truck Simulator 2/profiles/
und liefert Name + Pfad zurück.

2. Save laden
Tauri lädt:

bash
Code kopieren
PROFILE/autosave/info.sii
und cached die Werte.

3. Werte bearbeiten
Geld → bank: money:

XP → profile: experience_points:

4. Save zurückschreiben
Die geänderten Werte werden überschrieben und ETS2 akzeptiert die neue Savegame-Struktur.

📌 Roadmap / To-Do-Liste
Die Roadmap ist nach Prioritäten sortiert:

1. Grundfunktionen (DONE / IN PROGRESS)
 Profilscanner

 Laden eines Profils

 Geld auslesen

 XP/Level auslesen

 Geld ändern und speichern

 XP ändern und speichern

 UI Feedback-System verbessern

 Fehlermeldungen einheitlich gestalten

2. Erweiterte Save-Daten
 LKW-Liste auslesen

 Anhänger auslesen

 Garagen & Standort

 Spielerstatistik (km, Aufträge, Firmenlevel)

 Firmenkapital & Fahrer verwalten

 Mod-support (optional)

3. UI & UX
 Dunkel-/Hellmodus

 Animationen & bessere Buttons

 Suchfeld für Profile

 Settings-Seite

 Versionsinfo direkt im Programm anzeigen

4. Release-Vorbereitung
 Installer (.exe) bauen

 Code Signing vorbereiten

 GitHub Releases automatisieren

 Wiki Dokumentation erstellen

📜 Changelog
v0.1.0 – 27.11.2025
Erste funktionsfähige Version:

Profilscanner implementiert

Geld & XP auslesbar

Geld & XP änderbar

Save-System erstellt

Grundlegende UI & Struktur

📦 Geplante Module
Parser & Save-API
Bessere .sii Parser-Engine

Unterstützung für verschlüsselte Saves

Automatische Backups vor jedem Schreiben

ETS2 Multiplayer (TruckersMP)
TMP-Profil-Handling (falls technisch möglich)

Modding-Integration
Datei-Struktur von Mods auswerten

Konflikterkennung

🧪 Entwicklung & Beiträge
Pull Requests sind willkommen.
Bitte einen separaten Branch verwenden:

php-template
Code kopieren
feature/<name>
bugfix/<problem>
Konventionen:

Rust: Standard Rustfmt

JS: Prettier

Commits nach Conventional Commits:

makefile
Code kopieren
feat:, fix:, docs:, refactor:, chore:
⚠️ Haftungsausschluss
Dieses Projekt ist nicht offiziell von SCS Software.
Nutzung erfolgt auf eigene Verantwortung.
Savegames können beschädigt werden, daher werden Backups empfohlen.

📄 Lizenz
MIT License
Du darfst den Code frei nutzen, erweitern und veröffentlichen, solange die Lizenz beiliegt.

💬 Kontakt
Projekt von xLieferant
YouTube / GitHub / Discord (folgt)
