# FreePass

Gestionnaire de mots de passe **local, mono-utilisateur**. App de bureau **Tauri 2** (Rust + SQLite,
single binary, zéro cloud) + front **React 19**, avec une **extension navigateur** qui pré-remplit les
identifiants. Les secrets sont **chiffrés au repos** ; un **mot de passe maître** au déverrouillage
dérive (Argon2id) la clé qui les déchiffre en mémoire.

> ⚠️ **Aucune récupération** : perdre le mot de passe maître = perdre le coffre. Sauvegardez le
> fichier coffre vous-même.

## Documents de cadrage
- [`DESIGN.md`](DESIGN.md) — modèle fonctionnel + sécurité (*le quoi*).
- [`CRYPTO_SPEC.md`](CRYPTO_SPEC.md) — contrat crypto figé v1 (on l'applique, on ne le réinterprète pas).
- [`THREAT_MODEL.md`](THREAT_MODEL.md) — failles F1–F15 du coffre local + mitigations.
- [`PLAN.md`](PLAN.md) — phases 0→9 + critères d'acceptation.
- [`CLAUDE.md`](CLAUDE.md) — guidage stack & règles pour le développement.

## Chaîne d'agents
Pilotée par les agents de `.claude/agents/` (adaptés de ZePass) :
`briefer` → `plan-keeper` → `developer` / `frontend` → `security-reviewer` + `tester` → `ship`
(+ `release` en Phase 9). **Appeler `briefer` en premier** pour toute nouvelle tâche.
