# Tauri + Vanilla

This template should help get you started developing with Tauri in vanilla HTML, CSS and Javascript.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)


Template created! To get started run:
cd ets2-tool
cargo tauri android init

For Desktop development, run:
cargo tauri dev

For Android development, run:
cargo tauri android dev

## Troubleshooting

### Linux (KDE Plasma / Wayland)

If the app window is blank on Wayland, run with:

```sh
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo tauri dev
```

You can prefix the same environment variables to the app launch command outside of dev as well.
