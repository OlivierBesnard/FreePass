# RELEASING.md — FreePass

Runbook de publication (PLAN Phase 9). ⚙️ = outillable/automatisable · 🧑 = action
**humaine** (custody de clé, soumission store, revue externe — jamais automatisée).

## 0. Pré-requis
- Rust stable, Node + pnpm, et le bundler de l'OS (Windows : WiX/NSIS via Tauri ;
  macOS : Xcode CLT ; Linux : `dpkg`/`appimagetool`).
- `pnpm install` à jour.

## 1. ⚙️ Icône
Déjà câblée (`tauri.conf.json` → `bundle.icon`). Pour la régénérer après édition de
`assets/app-icon.svg` :
```powershell
node scripts/render-icon.mjs        # assets/app-icon.svg -> assets/app-icon.png (1024)
pnpm tauri icon assets/app-icon.png # régénère src-tauri/icons/*
```

## 2. ⚙️ Vérifications pré-build (doivent être vertes)
```powershell
pnpm typecheck
pnpm test                                   # Vitest (si présents)
pnpm build                                  # bundle front
cargo test --manifest-path src-tauri/Cargo.toml
```

## 3. 🧑 Clé de signature de l'updater (une fois, custody humaine)
```powershell
pnpm tauri signer generate -w "$HOME/.freepass/updater.key"
```
- Conserver la **clé privée hors dépôt** (gestionnaire de secrets / coffre matériel).
- Coller la **clé publique** dans `tauri.conf.json` → `plugins.updater.pubkey`, et
  renseigner `plugins.updater.endpoints` (URL du `latest.json`). *(Non committé tant
  que la clé n'existe pas — pas de pubkey factice qui casserait l'updater.)*

## 4. ⚙️ Build signé
```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$HOME/.freepass/updater.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<passphrase>"
pnpm tauri build
```
Artefacts : `src-tauri/target/release/bundle/` (installeur + binaire + signatures
updater `.sig`). Bump `version` dans `package.json`, `src-tauri/Cargo.toml`,
`src-tauri/tauri.conf.json` au préalable.

## 5. ⚙️ Empreintes
Générer `SHA256SUMS` des artefacts distribués et les publier à côté des binaires.

## 6. Extension navigateur
- ⚙️ Avant publication : ajouter `webextension-polyfill` + les icônes (reprendre
  `assets/app-icon.png`), figer la version dans `extension/manifest.json`, zipper
  `extension/`.
- 🧑 Soumission **Chrome Web Store** et **AMO** (comptes développeur, revue store).
  Figer le `gecko.id` AMO une fois publié.

## 7. 🧑 Go-live
- Vérifier le **README** : avertissement « aucune récupération » + sauvegarde du
  fichier coffre (présent).
- Smoke-test E2E sur la pile assemblée : créer le coffre → déverrouiller → ajouter
  un identifiant → appairer l'extension → autofill.
- 🧑 (Optionnel mais recommandé) **revue cryptographique externe** avant diffusion large.

## Rappels sécurité (ne pas contourner)
- Aucun secret committé : pas de clé privée d'updater, pas de `vault.sqlite` réel,
  pas d'identifiants store en dur (env/CI uniquement).
- Le packaging n'ouvre aucun chemin réseau pour un secret : canal **loopback only**.
- Ne pas toucher au module crypto pendant la release (cf. agent `release`).
