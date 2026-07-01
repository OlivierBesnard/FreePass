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
7. **Accès agent IA = futur (Phase 11), pas v1.** Cible **non-supervisée** (clé d'accès déchiffrant un
   environnement sans mot de passe maître), scoping **par environnement**. Seule l'architecture est
   posée en v1 ; voir DESIGN §10, CRYPTO_SPEC §9, THREAT_MODEL F16–F20. *(Renumérotée : la Phase 10
   accueille désormais « Projets & environnements ». DESIGN/CRYPTO_SPEC/THREAT_MODEL/SECURITY disent
   encore « Phase 10 » pour l'agent IA — référence à réaligner hors PLAN.)*
8. **Projets & environnements (Phase 10)** : calque **projet** (métadonnée claire) au-dessus des
   environnements + émergence du **multi-environnement** dans l'UI. **100 % additif, zéro changement
   crypto** : la hiérarchie `masterKey → vaultKey → envKey` est inchangée (CRYPTO_SPEC §3 reste figé).

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

## Phase 10 — Projets & environnements
> **Modèle choisi (acté 2026-06-30)** : `Projet → Environnement → entrées indépendantes`. Le **projet**
> est un nouveau niveau de regroupement **purement métadonnée claire** au-dessus des environnements ;
> chaque environnement garde ses **propres entrées** (pas de crédential logique transverse). Cette phase
> fait aussi **émerger le multi-environnement dans l'UI** (jusqu'ici masqué derrière l'env par défaut).
> **Décision structurante : AUCUN changement crypto.** La hiérarchie reste `masterKey → vaultKey →
> envKey` ; les entrées restent chiffrées sous l'`envKey` de **leur** environnement ; l'AAD est inchangé
> (`env_id`+`entry_id`+`field_name`). C'est précisément l'esprit de l'indirection figée
> (DESIGN §4/§10, CRYPTO_SPEC §3) : l'ajout est **additif, pas migratoire**.

**Objectif** : pouvoir regrouper plusieurs environnements (dev/staging/prod…) sous un même **projet**,
créer/renommer/archiver projets et environnements, et naviguer entre eux ; **sans re-chiffrer** quoi que
ce soit et **sans casser l'autofill** (le navigateur ignore projets/envs).

> **Raffinement UX (acté 2026-06-30)** : le **point d'entrée** de l'app est une **liste unifiée** qui
> montre les entrées de **tous les environnements vivants**, **regroupées par site** (domaine
> enregistrable, regroupement calculé **côté front**). L'**environnement n'est plus un préalable de
> navigation** : il devient un **badge optionnel** sur chaque entrée et reste la **fondation crypto**
> conservée (`envKey` par environnement, indirection figée). Le sélecteur projet → environnement et les
> écrans de gestion restent disponibles pour scoper/organiser, mais l'utilisateur n'a plus à choisir un
> environnement pour voir ses mots de passe. **Toujours zéro changement crypto** : la liste unifiée lit
> de la **métadonnée claire uniquement** (titre/url/nom d'env), exactement comme `list_entries` — aucun
> déchiffrement, aucune `envKey`.

**Périmètre — IN** :
- **Migration additive 004** : table `projects (id, name, created_at, updated_at, archived_at)` ;
  `ALTER TABLE environments ADD COLUMN project_id TEXT REFERENCES projects(id)` (**nullable**). Aucune
  colonne de principal (cf. invariant mono-user).
- **Backfill au démarrage (Rust)** : créer un projet par défaut « Personnel » et y rattacher tout
  environnement orphelin (`project_id IS NULL`). Idempotent. **Aucun re-chiffrement.**
- **Création d'environnement** : générer une `envKey` fraîche (OsRng) emballée sous la `vaultKey`,
  **en réutilisant** `crypto::wrap_env_key` + la logique de `services/vault.rs` (zéro primitive nouvelle).
- **Commandes IPC** (contrat figé en annexe) : `create_project` / `list_projects` / `rename_project` /
  `archive_project` ; `create_environment` / `list_environments` / `rename_environment` /
  `archive_environment` ; **`list_all_entries`** (liste unifiée multi-env, métadonnée claire) ; mise à
  jour de `credentials_for_origin` (balayage **multi-env**).
- **UI (FR)** : **liste unifiée par site** comme écran d'accueil (alimentée par `list_all_entries`,
  **regroupement par domaine enregistrable calculé côté front**), avec l'environnement en **badge
  optionnel** sur chaque entrée ; sélecteur projet → environnement et écrans de gestion
  (créer/renommer/archiver) disponibles pour scoper/organiser ; les vues d'entrées scopées opèrent sur
  l'`env_id` sélectionné (au lieu du seul env par défaut).

**Périmètre — OUT** (hors phase, anti scope creep) :
- Déplacer une entrée d'un environnement à un autre (impliquerait un re-chiffrement sous une autre
  `envKey`) → évolution future.
- Déplacer un environnement d'un projet à un autre au-delà d'un simple `UPDATE project_id` (pas de
  réorganisation crypto). *(Le re-parentage métadonnée pur peut rester OUT en v1 si non demandé.)*
- Types d'entrée `secret` / `env_var`, partage, hiérarchie de projets imbriqués, clés d'accès agent
  (→ Phase 11). Aucun nouveau champ chiffré.

**Critères d'acceptation (testables)** :
1. **Migration 004 additive** : appliquée automatiquement au démarrage sur une base existante ; les
   environnements et entrées **préexistants restent lisibles** (déchiffrement OK, aucune `envKey`
   re-générée). `cargo test` et `pnpm typecheck` au vert.
2. **Invariant mono-user préservé** : `migrations_do_not_reference_multi_user_columns` **passe**
   (`project_id` est un identifiant d'objet, jamais un principal ; aucun `user_id`/`owner_id`/
   `tenant_id`/`created_by`). DESIGN §6.
3. **Backfill idempotent** : au 1er démarrage post-migration, un projet « Personnel » existe et tout
   environnement orphelin y est rattaché ; un 2ᵉ démarrage ne crée pas de doublon.
4. **CRUD projet** : `create_project` / `list_projects` / `rename_project` / `archive_project`
   fonctionnent ; `list_projects` n'expose **que** les projets non archivés (sauf demande explicite) ;
   archiver est un **soft-delete** (`archived_at`), réversible côté données.
5. **CRUD environnement** : `create_environment(project_id, name)` génère **une `envKey` fraîche
   (OsRng)** emballée sous la `vaultKey` (vérifiable : la nouvelle ligne `environments` a un
   `env_key_wrapped` distinct, qui s'unwrap sous la `vaultKey` et **uniquement** avec le bon `env_id` —
   anti-swap F8) ; `list_environments(project_id)` liste les environnements non archivés du projet ;
   rename/archive opèrent en métadonnée pure.
6. **Crypto inchangée** : aucune modification de `crypto/`, de l'AAD, ni de `entry_fields` ; le test
   `change_master_password_keeps_vault_key_and_invalidates_old` et les vecteurs crypto restent verts.
   Toute entrée d'un environnement créé en Phase 10 chiffre/déchiffre comme l'env par défaut.
7. **Autofill multi-env (CRITIQUE)** : `credentials_for_origin` balaie **tous les environnements non
   archivés** (plus seulement l'env par défaut) et renvoie les identifiants matchant l'origine, quel
   que soit leur environnement/projet. Le **match d'origine strict (eTLD+1)** est inchangé (F6) ; coffre
   verrouillé ⇒ rien (F7). Test : une entrée créée dans un **second** environnement est autofillée.
8. **Coffre verrouillé** : toutes les commandes projet/environnement exigent l'unlock (fail-closed
   `VaultLocked`), comme les commandes entries existantes.
9. **Liste unifiée par site** : `list_all_entries` renvoie les entrées de **tous** les environnements
   vivants (env non archivé ET projet non archivé), métadonnée claire only (aucun secret, F5), chaque
   ligne portant `env_name` ; recherche locale sur `title`/`url` ; fail-closed `VaultLocked` si verrouillé.
   Test : des entrées de **deux** environnements apparaissent toutes avec le bon `env_name` ; une entrée
   d'un environnement OU projet archivé est exclue.
10. **Texte UI en français**, identifiants techniques/commentaires en anglais ; UUID v4, RFC3339,
   soft-delete via `archived_at`.

**Mitigations 🔒 rattachées** : F1/F2 (aucune `envKey`/clé en clair — les nouvelles `envKey` sont
emballées comme l'existante), F6 (match d'origine strict inchangé), F7 (canal sert seulement déverrouillé),
F8 (AAD liant `env_id` ⇒ une `envKey` ne déchiffre pas un autre environnement). **Invariant mono-user**
(DESIGN §6).

### Annexe — CONTRAT IPC FIGÉ (Phase 10)
> Référence unique pour `developer` (Rust) et `frontend` (TS). Conventions inchangées : noms de
> commandes en **snake_case** (Rust `#[tauri::command]`) ; les arguments d'`invoke` sont en
> **camelCase** (Tauri convertit `snake_case` Rust → `camelCase` JS) ; `AppError` sérialisé en string ;
> tout passe par `src/lib/api.ts`. Les structs de réponse dérivent `Serialize` (camelCase **non** forcé
> ⇒ champs `snake_case` côté JSON, comme `EntrySummary.env_id` / `EnvironmentInfo` aujourd'hui).

**Structs Rust (à ajouter dans `models/`)**
```rust
// models/project.rs
#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub id: String,            // UUID v4
    pub name: String,          // clear metadata (FR libre)
    pub created_at: String,    // RFC3339
    pub updated_at: String,
}

// EnvironmentInfo (models/entry.rs) — ÉTENDU : ajouter project_id.
#[derive(Debug, Serialize)]
pub struct EnvironmentInfo {
    pub id: String,
    pub name: String,
    pub project_id: Option<String>,   // NEW — nullable le temps du backfill
}
```

**Types TS (à ajouter/étendre dans `src/lib/api.ts`)**
```ts
export interface ProjectInfo {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
}
// EnvironmentInfo étendu :
export interface EnvironmentInfo {
  id: string;
  name: string;
  project_id: string | null;   // NEW
}
```

**Commandes — projets**
| Commande Rust | Wrapper api.ts | Params (`invoke`) | Réponse |
|---|---|---|---|
| `create_project(name: String)` | `createProject(name)` | `{ name }` | `ProjectInfo` |
| `list_projects()` | `listProjects()` | `{}` | `ProjectInfo[]` (non archivés, tri `name COLLATE NOCASE`) |
| `rename_project(project_id, name)` | `renameProject(projectId, name)` | `{ projectId, name }` | `void` |
| `archive_project(project_id)` | `archiveProject(projectId)` | `{ projectId }` | `void` (soft-delete `archived_at`) |

**Commandes — environnements**
| Commande Rust | Wrapper api.ts | Params (`invoke`) | Réponse |
|---|---|---|---|
| `create_environment(project_id, name)` | `createEnvironment(projectId, name)` | `{ projectId, name }` | `EnvironmentInfo` (génère `envKey` fraîche emballée sous `vaultKey`) |
| `list_environments(project_id)` | `listEnvironments(projectId)` | `{ projectId }` | `EnvironmentInfo[]` (non archivés du projet) |
| `rename_environment(env_id, name)` | `renameEnvironment(envId, name)` | `{ envId, name }` | `void` |
| `archive_environment(env_id)` | `archiveEnvironment(envId)` | `{ envId }` | `void` (soft-delete `archived_at`) |

**Commande — liste unifiée (NEW)**
| Commande Rust | Wrapper api.ts | Params (`invoke`) | Réponse |
|---|---|---|---|
| `list_all_entries(search: Option<String>)` | `listAllEntries(search?)` | `{ search }` | `EntrySummary[]` (tous les environnements **vivants**, métadonnée claire only, tri `title COLLATE NOCASE`) |

- Renvoie les entrées de **tous** les environnements non archivés dont le **projet** (s'il existe) n'est
  pas archivé non plus — même cadre d'exclusion que `credentials_for_origin`. **Aucun déchiffrement,
  aucune `envKey`** : métadonnée claire uniquement, comme `list_entries` (F5). Fail-closed `VaultLocked`
  si le coffre est verrouillé. Chaque ligne porte `env_name` (nom clair de l'environnement) pour le badge
  optionnel ; le **regroupement par domaine** est calculé **côté front**.
- **`EntrySummary` étendu** : ajout de `env_name: Option<String>` (snake_case en JSON, comme `env_id`).
  Les chemins par environnement (`list_entries`, `list_archived_entries`) laissent `env_name = null` ;
  seul `list_all_entries` le remplit.

**Commandes existantes — impact**
- `default_environment()` → **conservée** (rétrocompat : renvoie le 1er env non archivé, désormais
  enrichi de `project_id`). Sert de sélection par défaut au 1er rendu. *Le `frontend` migre l'UI vers la
  liste unifiée + sélection projet/env mais ne supprime pas la commande dans cette phase.*
- `credentials_for_origin()` (dans `services/local_channel.rs`, **pas** une commande IPC) → **MODIFIÉE** :
  itère sur **tous** les environnements non archivés (`SELECT id FROM environments WHERE archived_at IS
  NULL`), charge chaque `envKey` via `vault::load_env_key`, et agrège les crédentials matchant l'origine.
  Signature publique inchangée. C'est le critère d'acceptation #7.
- Commandes `entries` (`list/get/create/update/...`) → **inchangées** : elles prennent déjà `env_id` en
  paramètre. Le `frontend` passe l'`env_id` **sélectionné** au lieu de celui de `default_environment`.

**Règles de validation (service layer)**
- `name` projet/environnement : trim non vide (sinon `AppError::Conflict("le nom est requis")`),
  cohérent avec la validation `title` des entrées.
- Toutes les commandes : fail-closed `VaultLocked` si le coffre n'est pas déverrouillé (création d'env
  nécessite la `vaultKey` pour emballer la nouvelle `envKey`).
- `create_environment` : `project_id` doit référencer un projet existant non archivé (sinon
  `AppError::NotFound`).

---

## Phase 11 — Accès agent IA scopé (FUTUR, hors v1)
> Réalisable sans migration grâce à l'indirection `envKey` posée en v1 (décision 6). À **re-cadrer
> (briefer) et re-figer (CRYPTO_SPEC §9)** avant de démarrer. Réf : DESIGN §10, THREAT_MODEL F16–F20.
> *(Anciennement « Phase 10 » ; renumérotée pour insérer « Projets & environnements ». Les références
> « Phase 10 » dans DESIGN/CRYPTO_SPEC/THREAT_MODEL/SECURITY pointent vers cette phase et restent à
> réaligner.)*
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
Accès agent IA (→ **Phase 11** ci-dessus), synchronisation multi-appareils, partage, TOTP/2FA intégré,
attachements chiffrés, audit de fuite (HIBP), déverrouillage biométrique/OS keychain, navigateurs
au-delà de Chrome/Firefox. À reconsidérer après dogfooding — ne pas laisser fuiter dans v1 (anti scope
creep, rôle `plan-keeper`).
