# Vintage Mod Manager (VMM)

A lightweight, minimalist desktop mod manager, modpack sharer, and multiplayer launcher for **Vintage Story**, built with **Tauri v2**, **Rust**, **React 19**, and **Tailwind CSS**.

---

## Features

### Mod Management & Mod DB
- **Installed Mods**: Enable or disable mods cleanly (moves archives between `Mods/` and `ModsDisabled/`), delete mods, and check for updates against `mods.vintagestory.at`.
- **Pre-Update Changelogs**: Clicking **Update** on an individual mod fetches release notes from Mod DB so you can review changes before downloading.
- **Mod DB Browser**: Search, sort, and filter mods by category tags or Vintage Story game version, inspect release history, and install directly.
- **Mod Config Editor**: Edit JSON files in `ModConfig/` directly inside VMM before launching the game.

### Profiles & Modpack Sharing
- **Mod Profiles**: Switch between curated mod sets without duplicating `.zip` files, override launch args/RAM/executable per profile, and pin specific mod versions.
- **Export & Share**: Export any profile as a `.vmmpack.json` manifest, a `.vmmpack.zip` bundle (optionally including `ModConfig/` files), or a compact `vmm1:...` share code.
- **Import & Remote Sync**: Import from a local file, `vmm1:...` share code, or an `https://` URL (such as a GitHub Raw or GitHub Gist URL). Profiles imported from a URL include a one-click **Sync from GitHub URL** button.

### Multiplayer & Encrypted P2P Rooms
- **Built-in P2P Tunnel (Zero Extra Downloads)**: Powered by `iroh` (QUIC + TLS 1.3), VMM lets you host and join multiplayer rooms using a `vmm-p2p:...` Room Code with a per-session secret—no VPNs, third-party accounts, or router port-forwarding required.
- **Dedicated Server Manager**: Auto-detects `VintagestoryServer.exe`, isolates server saves/configs in `<dataPath>/VMM_Server`, generates valid default `serverconfig.json` via `--genconfig`, syncs your active profile's mods via `--addModPath`, and streams a live interactive server console.
- **In-Game "Open to LAN" Mode**: Share your singleplayer world over the built-in P2P tunnel without running a separate dedicated server.

### Built-in App Updates
- Automatically checks **GitHub Releases** (`mxltreated/VMM`, configurable in **Settings**) on startup and can download and launch the latest `.exe` / `.msi` installer directly from inside VMM.

---

## Tech Stack
- **Backend**: Rust, Tauri v2, Reqwest, Tokio, `zip`, `flate2`, `semver`
- **Frontend**: React 19, TypeScript, Vite, Tailwind CSS v4, Lucide Icons

---

## Development

### Prerequisites
- [Node.js](https://nodejs.org/) (v20+)
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- Windows WebView2 / Tauri v2 build prerequisites

### Commands
```powershell
# Install frontend dependencies
npm install

# Run in development mode (frontend + Tauri window)
npm run tauri dev

# Typecheck & build frontend
npm run typecheck
npm run build

# Run Rust unit tests
cargo test --manifest-path src-tauri/Cargo.toml

# Build production installer (.exe / .msi)
npm run tauri build
```

---

## Publishing Releases (GitHub Actions)
This repository includes [.github/workflows/release.yml](.github/workflows/release.yml). Pushing a version tag (e.g. `v1.1.0`) automatically builds the Windows installers and publishes a GitHub Release:

```powershell
git tag v1.1.0
git push origin v1.1.0
```

---

## License
See [LICENSE](LICENSE) for details.
