# Project Overview

This is the ETS2 Save Edit Tool – Alex Edition, a desktop application designed to modify Euro Truck Simulator 2 (ETS2) profiles, truck settings, and game preferences. It's an open-source hobby project focused on learning and customization.

The application is built using the [Tauri framework](https://tauri.app/), which allows for building cross-platform desktop applications with a Rust backend and a web-based frontend.

## Key Features:
*   **Truck Management:** Repair trucks, adjust fuel levels.
*   **Trailer Management:** Basic structure implemented, with advanced editing features planned.
*   **Profile Editing:** Modify in-game money, XP, and driver skill levels.
*   **Game Settings:** Toggle various game settings like Convoy Mode, Traffic Density, Parking Doubles, and Developer Mode.

## Architecture
The project follows a client-server architecture:
*   **Frontend:** A web-based user interface (`ets2-tool/src/`) built with HTML, CSS, and JavaScript. It communicates with the backend via Tauri's inter-process communication (IPC).
*   **Backend:** A Rust application (`ets2-tool/src-tauri/src/`) that handles core logic such as reading and writing to ETS2 save files, performing data conversions (e.g., float to hex), and managing system interactions.

# Building and Running

This project uses [Tauri](https://tauri.app/) to build a cross-platform desktop application. To build and run the application, you will need Node.js (with npm or yarn) and Rust (with Cargo) installed.

### Prerequisites:
*   **Node.js**: [Download & Install Node.js](https://nodejs.org/) (includes npm)
*   **Rust**: [Install Rust](https://www.rust-lang.org/tools/install)
*   **Tauri CLI**: Install globally using `npm install -g @tauri-apps/cli` or `yarn global add @tauri-apps/cli`.

### Development:

To run the application in development mode:

```bash
# Navigate to the root directory of the project
npm install # Or yarn install, to install frontend dependencies
npm run tauri dev # Or yarn tauri dev
```

### Building for Production:

To build a production-ready application installer:

```bash
# Navigate to the root directory of the project
npm install # Or yarn install
npm run tauri build # Or yarn tauri build
```
The generated installers/executables will be found in `ets2-tool/src-tauri/target/release/bundle/`.

# Development Conventions

*   **Rust:** Adhere to idiomatic Rust practices and formatting as enforced by `rustfmt`.
*   **JavaScript/Frontend:** Follow standard JavaScript best practices. The project appears to use vanilla JavaScript with a structured module approach.
*   **Project Structure:** Maintain the existing separation between frontend (`ets2-tool/src/`) and backend (`ets2-tool/src-tauri/src/`) components.
*   **Tauri IPC:** When adding new features, leverage Tauri's command system for communication between the frontend and backend.

# Key Files

*   `README.md`: Project overview and user-level instructions.
*   `ets2-tool/package.json`: Frontend JavaScript dependencies.
*   `ets2-tool/src-tauri/Cargo.toml`: Backend Rust dependencies and project metadata.
*   `ets2-tool/src-tauri/src/main.rs`: Main entry point for the Rust backend, defining Tauri commands.
*   `ets2-tool/src-tauri/src/shared/hex_float.rs`: Contains utility functions for converting between floats and the SII hex float format used in save files.
*   `ets2-tool/src-tauri/src/features/vehicles/editor.rs`: Logic for editing vehicle-related data (trucks and trailers) in save files.
*   `ets2-tool/src/app.js`: Core frontend application logic, including modal management.
*   `ets2-tool/src/tools.js`: Defines the UI tools and their corresponding actions, invoking backend commands.
