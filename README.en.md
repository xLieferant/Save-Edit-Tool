# ETS2 Save Edit Tool  
A modern and powerful savegame editor for Euro Truck Simulator 2 (ETS2) and later American Truck Simulator (ATS).  
Current status: **0.1 Alpha (in development)**

---

## Overview
The tool reads ETS2 savegames, displays data, allows editing, and writes changes back safely.  
Unavailable features appear as “Coming soon”.

Updates are delivered through a built-in auto-updater using GitHub Releases.

## 🚀 Upcoming in v0.2.0
We are currently working on a major UI/UX overhaul to make the experience faster and more modern.

- **Modernized UI:** Clean, structured layout with a new tab-based navigation.
- **Theme Support:** Choose between three curated themes: *Neon Night*, *Dark Pro*, and *Pure Light*.
- **Performance:** Optimized core logic for a snappier feel and faster load times.
- **Quality of Life:** Improved modal handling, contextual help texts, and status indicators.
- **Community Focus:** Integrated update checker and direct GitHub feedback links.


---

## Features in Version 0.1 Alpha

### UI / Modals
- Dynamic modals  
- Working Save & Cancel  
- Disabled “Coming soon” modals  
- Base navigation (Truck, Trailer, Profile, Settings)

### Truck
- Repair truck  
- Edit fuel level  
- Switch fuel tank  
- Edit mileage  
- Edit license plate  

### Trailer
- Repair trailer  
- Edit trailer license plate  
- Edit job weight

### Profile
- Edit XP  
- Edit money  
- Edit skills and experience  
- View and modify stats (where stable)

### Settings
- Basic color theme  
- Convoy 128  
- Traffic value  
- Parking doubles  
- Language placeholder  
- Dev mode

---

## Roadmap

### Version 0.2
- Improved GUI  
- Color theme picker  
- Better navigation  
- Early quality-of-life features  

### Version 0.3
- ATS support  
- Truck, trailer, profile parsing  
- Shared engine for ETS2 & ATS  

### Version 0.4
- Language updater  
- 0.4.1 English release  
- 0.4.2 German release  
- Community languages

### Version 0.5+
- AI driver editing  
- Map unlock tools (cities, countries, roads)  
- Clone profile  
- Rename profile  
- Select save folder  
- Economy tools  
- Repair all trucks / repair all trailers  

---

## Installation & Updates
Delivered as MSI installer.  
The auto-updater checks GitHub Releases for new versions.

Only published releases trigger updates.  
Regular commits do **not** create update notifications.

---

## Contributing
Feel free to open issues for ideas, bugs, or feature requests.  
Pull requests are welcome, especially for translations and new features.


## UI and localization checks

The shared desktop design tokens are defined in `ets2-tool/src/styles/design_system.css`.
All module pages load this stylesheet after their layout styles. The default theme
uses neutral dark surfaces, subtle glass panels and visible keyboard focus states.
Existing theme choices remain available.

Run these checks from `ets2-tool/` with Node.js:

```powershell
node scripts/check_i18n.mjs
node scripts/check_i18n.mjs --json
node scripts/check_ui.mjs
```

The localization check validates German key coverage, literal source references,
translation markers and interpolation placeholders. Its JSON report includes keys
without detected references and missing entries in other languages. Dynamic key
families require review; an unassigned candidate is not proof of unused text.
Other languages continue to use the existing English fallback.

The UI check uses an installed Chromium browser, set with `CHROME_PATH` when needed.
It checks seven pages in German and English at 900 x 640, 1210 x 800 and 1600 x 1000,
including translation switching, placeholder interpolation, hidden states, page
overflow and enabled-button text contrast. It uses read-only Tauri fixtures and
writes screenshots to a temporary directory. Native file dialogs and real savegame
writes require separate desktop testing.

The implementation report and file list are in [UI_REVIEW.md](ets2-tool/UI_REVIEW.md).
