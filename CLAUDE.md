# CLAUDE.md — FreePass

Guidance pour Claude Code sur ce dépôt. Ces instructions priment sur le comportement par défaut.

## Statut du dépôt

**FreePass** est un **gestionnaire de mots de passe local, mono-utilisateur** : app de bureau
**Tauri 2** (Rust + SQLite, single binary, zéro cloud, zéro auth serveur) + front **React 19**, avec
une **extension navigateur (MV3)** qui pré-remplit les identifiants. Les secrets sont **chiffrés au
repos** dans SQLite ; un **mot de passe maître** au déverrouillage dérive (Argon2id) la clé qui les
déchiffre **en mémoire**. **Pas** de zero-knowledge réseau, **pas** de serveur, **pas** de récupération
du mdp maître.

Le projet démarre du scaffold. Le cadrage vit dans 4 docs racine, à lire avant de coder :
- [`DESIGN.md`](DESIGN.md) — modèle fonctionnel + sécurité (*le quoi*).
- [`CRYPTO_SPEC.md`](CRYPTO_SPEC.md) — **contrat crypto figé v1** (le plus important — on l'applique).
- [`THREAT_MODEL.md`](THREAT_MODEL.md) — failles **F1–F15** du coffre local + mitigations.
- [`PLAN.md`](PLAN.md) — phases 0→9 + critères d'acceptation.

## Stack

- **Shell desktop** : Tauri 2.x (backend Rust + webview, single binary, no cloud).
- **Front** : React 19 + TypeScript strict + Vite, Tailwind v4 (`@theme`), design **Studio** (palette
  terracotta + Fraunces + boussole, repris de `freelance-savior`), React Router 7, TanStack Query
  (server state) + Zustand (UI state), React Hook Form + Zod, lucide-react, sonner.
- **Backend Rust (`src-tauri/`)** : sqlx (SQLite + migrations), serde, uuid, chrono, thiserror, tokio.
- **Crypto** : **RustCrypto pur** — `argon2`, `chacha20poly1305` (`XChaCha20Poly1305`), `zeroize`,
  `rand`/`getrandom` (`OsRng`). **Aucune crypto maison, aucune dépendance C.** Cf. `CRYPTO_SPEC.md`.
- **Extension** : MV3 + `webextension-polyfill`. Canal app↔extension = **serveur loopback 127.0.0.1**
  + token d'appairage.
- **Stockage** : SQLite dans l'app-data OS (`%APPDATA%\com.freepass.desktop\vault.sqlite` sur Windows).

## Commandes (prévisionnelles)

```powershell
pnpm tauri dev        # app desktop en dev
pnpm tauri build      # build prod signé
pnpm test             # Vitest (front)
pnpm typecheck        # tsc --noEmit
cargo test --manifest-path src-tauri/Cargo.toml    # tests Rust (dont crypto + vecteurs)
cargo check --manifest-path src-tauri/Cargo.toml
```

Les migrations sqlx s'exécutent **automatiquement au démarrage** (`sqlx::migrate!`), jamais à la main.

## Architecture (cible)

Deux moitiés via l'IPC `invoke()` de Tauri :
1. **`src/` (React)** — pages sous `src/pages/`, composants `src/components/{ui,layout,shared,...}`,
   IPC via `src/lib/api.ts`, schémas Zod `src/lib/schemas.ts`, hooks `useX()` par domaine.
2. **`src-tauri/src/`** — `models/` (structs sqlx), `commands/` (handlers `#[command]`), `services/`
   (logique : `crypto`, `vault`, `local_channel`), `db/` (pool + migrations), `error.rs`
   (`AppError` sérialisable), `state.rs` (`AppState`).

Conventions : IDs = UUID v4 ; timestamps = RFC3339 (TEXT) ; soft-delete via `archived_at`.

## Règles non négociables (sécurité)
- 🔒 **Mdp maître / clés jamais persistés en clair, jamais loggés, jamais dans une erreur/IPC.** Clés
  en mémoire process, effacées (`zeroize`) au lock/timeout/quit. (THREAT F2, F3, F5)
- 🔒 **On applique `CRYPTO_SPEC.md` à la lettre** : hiérarchie **3 niveaux** `masterKey → vaultKey →
  envKey` (entrées chiffrées sous l'`envKey` de leur **environnement**), Argon2id ≥ plancher,
  XChaCha20-Poly1305, **AAD** liant `env_id`+`entry_id`+`field_name`, **nonce 24 o aléatoire frais**,
  **OsRng** partout. Aucune crypto maison. L'indirection `envKey` est une décision v1 structurante
  (elle rend l'**accès agent IA futur** additif — DESIGN §10, CRYPTO_SPEC §9 ; ne pas la retirer).
- 🔒 **Canal extension loopback only** + token d'appairage + vérif origine ; secrets servis seulement
  coffre déverrouillé. (THREAT F7, F14)
- 🔒 **Autofill : match d'origine strict (eTLD+1)**, jamais cross-origin ni silencieux. (THREAT F6)
- **Invariant mono-utilisateur** : jamais de `user_id`/`owner_id`/`tenant_id`/« current user ». Un test
  de garde scanne les migrations (DESIGN §6). Ne pas l'affaiblir.

## Conventions de texte
- **Tout texte UI visible en français** (labels, placeholders, toasts, erreurs).
- **Commentaires & identifiants techniques en anglais** (tables, types, hooks, slugs).

## Chaîne d'agents

Le projet est piloté par une **chaîne d'agents** (dans `.claude/agents/`), adaptée de ZePass :
`briefer` → `plan-keeper` → `developer` / `frontend` → `security-reviewer` + `tester` → `ship`
(+ `release` pour la Phase 9). Appeler **briefer en premier** pour toute nouvelle tâche.
