# PLAN.md — FreePass

> Découpage en **phases** avec livrables et **critères d'acceptation** vérifiables. C'est la feuille
> de route du `plan-keeper` et l'entrée du `developer`/`frontend`. Sources de vérité associées :
> [`DESIGN.md`](DESIGN.md) (fonctionnel + sécurité), [`CRYPTO_SPEC.md`](CRYPTO_SPEC.md) (contrat figé),
> [`THREAT_MODEL.md`](THREAT_MODEL.md) (F1–F15).
>
> Convention : `[ ]` à faire · `[~]` en cours · `[x]` fait. Chaque phase passe par la chaîne
> briefer → plan-keeper → developer/frontend → security-reviewer → tester → ship.

## Décisions de cadrage actées (2026-06-25)
1. **Stack** : Tauri 2 (Rust + SQLite) + React 19/TS/Vite/Tailwind v4, design **Studio** (ex
   freelance-savior). **Mono-utilisateur, local, zéro cloud.**
2. **Modèle sécu** : coffre **chiffré au repos**, mdp maître → Argon2id → `vaultKey` → AEAD par champ.
   **Pas** de zero-knowledge réseau, **pas** de serveur, **pas** de récupération.
3. **Crypto** : **RustCrypto pur** (argon2, chacha20poly1305, zeroize, rand).
4. **Canal extension** : serveur **loopback 127.0.0.1** + token d'appairage.
5. **v1** = coffre CRUD + déverrouillage + extension autofill + générateur + recherche/Cmd+K + import CSV.
6. **Modèle de clés à 3 niveaux dès v1** (`masterKey → vaultKey → envKey`) : les entrées appartiennent
   à un **environnement** et sont chiffrées sous l'`envKey` de celui-ci. v1 expose au moins un
   environnement par défaut. Raison : rendre l'**accès agent IA scopé** (point 7) **additif** plus tard.
7. **Accès agent IA = futur (Phase 10), pas v1.** Cible **non-supervisée** (clé d'accès déchiffrant un
   environnement sans mot de passe maître), scoping **par environnement**. Seule l'architecture est
   posée en v1 ; voir DESIGN §10, CRYPTO_SPEC §9, THREAT_MODEL F16–F20.

---

## Phase 0 — Bootstrap & invariants
**Livrables** : scaffold Tauri 2 + React/Vite/Tailwind v4 ; tokens design Studio dans `index.css` ;
sqlx + dossier `migrations/` (migration auto au démarrage) ; `AppError` sérialisable ; `AppState`
(`pool`, `app_data_dir`) ; test de garde **mono-utilisateur** sur les migrations.
**Critères d'acceptation** :
- `pnpm tauri dev` lance une fenêtre vide stylée Studio ; `cargo check` et `pnpm typecheck` passent.
- Une migration crée la base dans `%APPDATA%\com.freepass.desktop\vault.sqlite`.
- Test `migrations_invariants` échoue si une migration contient `user_id`/`owner_id`/`tenant_id`/… (DESIGN §6).

## Phase 1 — Cœur crypto (Rust, sans UI)
**Livrables** : module `crypto/` : Argon2id KDF (params §2 CRYPTO), wrap/unwrap `vaultKey`, seal/open
de champ avec AAD, types `Zeroize`. **Vecteurs de test**.
**Critères** :
- Tests verts : KDF déterministe (sel fixe), wrap→unwrap = identité, seal→open = identité.
- Tests d'**échec attendu** : altération 1 octet de ciphertext / nonce / AAD / `entry_id` ⇒ erreur.
- Refus si `argon2_params` < plancher. Aucun `unwrap()` sur du matériel secret en prod. 🔒 F4, F8, F10, F12.

## Phase 2 — Coffre : init, déverrouillage, verrouillage
**Livrables** : schéma SQLite (`vault` 1 ligne + `environments` + `entries` avec `env_id`) ; création
de l'**environnement par défaut** à l'init + emballage de son `envKey` ; commandes `create_vault`,
`unlock`, `lock`, `change_master_password` ; état déverrouillé en mémoire (`vaultKey`/`envKey`
zeroizables) ; écrans React **Créer le coffre** / **Déverrouiller** (design Studio, FR).
**Critères** :
- Créer un coffre puis déverrouiller avec le bon mdp réussit ; mauvais mdp ⇒ erreur **générique** (anti-oracle).
- `vaultKey`/`envKey`/`masterKey` absentes du disque (inspection `vault.sqlite`) ; effacées au lock. 🔒 F1, F2, F3, F5.
- `change_master_password` n'altère ni les entrées ni les `envKey` (ré-emballe seulement) et invalide l'ancien mdp.
- L'indirection `envKey` est en place (CRYPTO_SPEC §3) même si l'UI n'expose qu'un environnement.

## Phase 3 — CRUD entrées + recherche/Cmd+K
**Livrables** : commandes `list/get/create/update/archive/delete_entry` (champs sensibles chiffrés
avant écriture, déchiffrés à la lecture en mémoire) ; UI liste + formulaire (RHF + Zod) ; recherche
locale + palette Cmd+K (sur `title`/`url` uniquement).
**Critères** :
- CRUD complet ; soft-delete (`archived_at`) puis purge ; recherche **100 % locale**.
- Inspection DB : `username`/`password`/`notes` illisibles ; `title`/`url` en clair (assumé). 🔒 F1, F5, F8.

## Phase 4 — Générateur de mots de passe
**Livrables** : générateur (longueur + classes maj/min/chiffres/symboles) via OsRng non biaisé ;
indicateur de force ; intégration au formulaire d'entrée.
**Critères** : génération reproductible de longueur/charset demandés ; aucune dépendance à un PRNG non
crypto ; pas de biais modulo. 🔒 F5.

## Phase 5 — Import CSV
**Livrables** : import d'un export navigateur (papaparse) → mapping colonnes → entrées chiffrées ;
avertissement « le CSV source est en clair ».
**Critères** : import d'un CSV Chrome/Firefox crée les entrées correctes ; confirmation + avertissement
affichés ; erreurs de format gérées. 🔒 F13.

## Phase 6 — Canal local (loopback) + appairage
**Livrables** : serveur HTTP/WS sur `127.0.0.1` (loopback only) démarré à l'unlock, coupé au lock ;
flux d'**appairage** (token capability validé par action UI) ; vérif origine + token sur chaque
requête ; endpoint « identifiants pour origine X » servi seulement si déverrouillé.
**Critères** :
- Le serveur n'écoute **pas** hors loopback (preuve : scan/`netstat`).
- Requête sans token ou mauvaise origine ⇒ refus ; requête coffre verrouillé ⇒ refus.
- Le token ne déverrouille pas le coffre. 🔒 F7, F14.

## Phase 7 — Extension navigateur MV3 (autofill)
**Livrables** : extension MV3 (`webextension-polyfill`) : détection champs login/mdp, **match
d'origine strict** (eTLD+1), popup listant les identifiants matchant l'onglet, autofill sur
confirmation ; appairage au canal ; UI minimale Studio.
**Critères** :
- Autofill propose **uniquement** les entrées du bon domaine ; jamais cross-origin, jamais silencieux. 🔒 F6.
- L'extension ne persiste que le token (pas de secret en `storage.local` durable). 🔒 F14.
- Coffre verrouillé ⇒ l'extension n'obtient rien.

## Phase 8 — Durcissement sécurité
**Livrables** : auto-lock par inactivité (config) ; effacement presse-papier après délai ; audit
`zeroize` sur tous chemins ; hygiène logs/erreurs ; passe complète THREAT_MODEL F1–F15.
**Critères** : `security-reviewer` **OK POUR SHIP** + `tester` au vert sur la liste d'attaques (IDOR
n/a, mais : brute-force param, swap/rollback, nonce, phishing autofill, canal non appairé, fuite log,
presse-papier). 🔒 F3, F5, F6, F7, F9, F11.

## Phase 9 — Release & packaging
**Livrables** : build Tauri signé + updater ed25519 (clé publique épinglée) ; packaging extension
(CWS/AMO) ; `README` utilisateur (dont **avertissement « aucune récupération » + sauvegarde du
coffre**) ; checklist de release.
**Critères** : build reproductible signé ; smoke-test E2E (créer→déverrouiller→ajouter→autofill) sur
la pile assemblée ; artefacts d'extension prêts à soumettre. 🔒 F15.

## Phase 10 — Accès agent IA scopé (FUTUR, hors v1)
> Réalisable sans migration grâce à l'indirection `envKey` posée en v1 (décision 6). À **re-cadrer
> (briefer) et re-figer (CRYPTO_SPEC §9)** avant de démarrer. Réf : DESIGN §10, THREAT_MODEL F16–F20.
**Livrables (cible)** : types `secret` / `env_var` + gestion d'**environnements** dans l'UI ; création
de **clés d'accès** scopées à un environnement (emballage de l'`envKey` sous la clé d'accès) ;
expiration + **révocation = rotation de l'`envKey`** ; **journal d'audit** des accès ; surface de
service locale (extension du canal loopback / **serveur MCP local** / **CLI** d'injection d'env) ;
fonctionnement **non-supervisé** (sans mot de passe maître).
**Critères (cible)** : une clé d'accès lit **uniquement** son environnement, app verrouillée OK ; clé
révoquée ⇒ accès **réellement** coupé (rotation prouvée) ; clé expirée ⇒ refus ; aucun secret hors
machine ; audit complet sans valeur de secret. 🔒 F16, F17, F18, F19, F20.

---

## Hors scope v1 (évolutions futures)
Accès agent IA (→ **Phase 10** ci-dessus), synchronisation multi-appareils, partage, TOTP/2FA intégré,
attachements chiffrés, audit de fuite (HIBP), déverrouillage biométrique/OS keychain, navigateurs
au-delà de Chrome/Firefox. À reconsidérer après dogfooding — ne pas laisser fuiter dans v1 (anti scope
creep, rôle `plan-keeper`).
