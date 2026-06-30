# DESIGN.md — FreePass

> Modèle fonctionnel et de sécurité de **FreePass**. C'est *le quoi* du produit.
> Source de vérité fonctionnelle. Le contrat crypto figé vit dans [`CRYPTO_SPEC.md`](CRYPTO_SPEC.md),
> les menaces dans [`THREAT_MODEL.md`](THREAT_MODEL.md), le découpage en phases dans [`PLAN.md`](PLAN.md).

## 1. ADN du produit

FreePass est un **gestionnaire de mots de passe local, mono-utilisateur**, pour le poste de
l'utilisateur. Il marie deux héritages :

- **L'expérience & la stack de `freelance-savior`** : application **Tauri 2** (Rust + webview),
  **single binary, 100 % local, zéro cloud, zéro auth serveur, mono-utilisateur**. Front
  **React 19 / TypeScript / Vite / Tailwind v4**, design system **Studio** (palette terracotta,
  Fraunces, boussole), **UI entièrement en français**.
- **La rigueur sécurité de `ZePass`**, mais en version **simplifiée** : **pas de zero-knowledge
  multi-utilisateur, pas de serveur, pas de Modèle B / wraps / multi-admin / IDOR**. À la place,
  un modèle **coffre local chiffré au repos** : les secrets sont chiffrés dans la base SQLite,
  un **mot de passe maître** au déverrouillage dérive la clé qui les déchiffre en mémoire.

S'ajoute une **extension navigateur (MV3)** qui pré-remplit les identifiants depuis le coffre.

### Ce que FreePass n'est PAS (périmètre négatif permanent)
- Pas de **synchronisation cloud**, pas de coffre partagé, pas de multi-appareils côté serveur.
- Pas de **multi-utilisateur / multi-tenant** : un coffre = une personne sur une machine.
  (Invariant mono-utilisateur, cf. §6.)
- Pas de **récupération / reset / break-glass** du mot de passe maître : le perdre = perdre le
  coffre. La continuité repose sur la **sauvegarde du fichier coffre** par l'utilisateur.
- Pas de crypto **maison** ni de crypto **côté réseau** : tout reste sur la machine.
- Pas d'**accès agent IA en v1** : la capacité de donner une clé d'accès scopée à un agent (§10) est
  une **direction future**. Seule l'**architecture** est mise en place dès v1 (indirection par
  environnement) ; la fonctionnalité ne l'est pas.

## 2. Acteurs et surfaces

| Surface | Rôle | Techno |
|---------|------|--------|
| **App de bureau** | Coffre, déverrouillage, CRUD, générateur, recherche, import | Tauri 2 (Rust) + React |
| **Extension navigateur** | Détection des champs + autofill | MV3 (Chrome/Firefox) |
| **Canal local** | Pont app ↔ extension | Serveur **loopback 127.0.0.1** + token d'appairage |

Un seul utilisateur humain. Aucun serveur distant. Aucune autre machine.

## 3. Modèle de sécurité (résumé — détail dans THREAT_MODEL.md)

### Frontière de confiance
- **Au repos** (fichier SQLite sur disque) : les champs sensibles (mot de passe, et tout champ marqué
  secret) sont **chiffrés** (AEAD). Un attaquant qui vole le fichier coffre **ne peut rien lire** sans
  le mot de passe maître.
- **Déverrouillé** (process en cours, coffre ouvert) : la clé de coffre et les secrets déchiffrés
  vivent **en mémoire du process** uniquement. On assume qu'un attaquant **root/malware sur la machine
  déverrouillée** sort du périmètre (limite assumée, comme tout gestionnaire local).
- **Mot de passe maître** : ne quitte **jamais** la machine, n'est **jamais** persisté, n'apparaît
  **jamais** dans un log/une erreur/une IPC. Il sert uniquement à dériver la clé (Argon2id) à
  l'ouverture, puis est effacé de la mémoire (`zeroize`).

### Attaquants considérés
- **A1 — Vol du fichier coffre** (disque, sauvegarde, backup cloud de l'OS) : doit rester illisible.
- **A2 — Site de phishing** : ne doit jamais récupérer un identifiant via l'autofill (match d'origine strict).
- **A3 — Application/malware local non-root** tentant de parler au canal loopback : doit être bloqué
  par le token d'appairage.
- **A4 — Lecture passive des logs/erreurs/presse-papier**.
- *Hors périmètre* : **A5 — root/malware actif sur machine déverrouillée** (limite assumée, documentée).

## 4. Modèle de données (fonctionnel)

Le coffre s'organise en **environnements** (`environments`), chacun contenant des **entrées**.
L'environnement est l'**unité de regroupement et de scoping** (c'est lui qu'une future clé d'accès
agent ciblera, cf. §10). En v1, il existe au moins un **environnement par défaut** (« Personnel ») ;
l'UI multi-environnements peut rester minimale, mais le **modèle de clés est déjà par environnement**
(CRYPTO_SPEC §3 — décision structurante pour ne pas avoir à re-chiffrer plus tard).

```
Environment {
  id         : UUID v4
  name       : nom lisible        (clair — métadonnée, ex. "Personnel", "projet-X-prod")
  created_at, updated_at, archived_at
}
```

Une **entrée** appartient à un environnement et a un **type** :

```
Entry {
  id          : UUID v4
  env_id      : → Environment.id
  type        : "login" | "secret" | "env_var"
  name/title  : libellé lisible           (clair — métadonnée)
  url         : domaine/URL associé        (clair — sert au match autofill ; type "login")
  -- champs CHIFFRÉS selon le type :
  login   ⇒ username, password, notes
  secret  ⇒ value, notes                   (clé API, token unique…)
  env_var ⇒ une paire { key (clair) : value (chiffré) }   (variable d'environnement type .env)
  created_at, updated_at, archived_at
}
```

- **Champs en clair** : `name`/`title`, `url`, `name` d'environnement, et la **clé** d'une variable
  (`API_KEY`) — nécessaires à la recherche, au match de domaine, et à l'injection d'env. **Fuite de
  métadonnée assumée** (THREAT F5) : ce ne sont pas des secrets ; les **valeurs** le sont.
- **Champs chiffrés** : `username`/`password`/`notes`/`value` — chiffrés sous la **`envKey` de leur
  environnement**, AEAD lié à `env_id` + `entry_id` + nom du champ (CRYPTO_SPEC §4 — anti-swap/rollback).
- Tables crypto : `vault` (params Argon2id, `vaultKey` emballée) + `environments` (chaque `envKey`
  emballée par `vaultKey`). Cf. CRYPTO_SPEC §3.

## 5. Parcours principaux

1. **Première ouverture** → écran « Créer le coffre » : l'utilisateur choisit un mot de passe maître
   (avec indicateur de force + avertissement « aucune récupération possible »). On génère sel +
   clé de coffre, on emballe la clé, on écrit le coffre verrouillé.
2. **Déverrouillage** (à chaque lancement / après auto-lock) → saisie du mdp maître → Argon2id →
   déballage de la clé de coffre. Échec = message **générique** (anti-oracle), pas de distinction
   « mauvais mdp » vs « coffre corrompu ».
3. **CRUD entrée** → ajout/édition/suppression (soft-delete puis purge). Champs sensibles chiffrés
   avant écriture SQLite.
4. **Générateur** → à la création/édition, génère un mdp fort (longueur + classes de caractères).
5. **Recherche / Cmd+K** → filtre **100 % local** sur `title`/`url`.
6. **Import CSV** → import d'un export navigateur ; avertissement que le CSV source est en clair.
7. **Autofill** → l'extension demande au canal local les identifiants matchant l'origine de l'onglet
   actif ; l'app répond (si déverrouillée et appairée) ; l'utilisateur confirme, l'extension remplit.
8. **Verrouillage** → manuel, à la fermeture, ou auto après inactivité → `zeroize` des clés/secrets,
   coupure du canal.

## 6. Invariant mono-utilisateur (ne pas casser)

Hérité de `freelance-savior` : **mono-utilisateur, sans auth serveur, sans multi-tenant**.
- **Jamais** de colonne `user_id` / `owner_id` / `tenant_id` / `created_by` dans une table ou migration.
- **Jamais** de notion de « current user » côté Rust ou React.
- Le « mot de passe maître » n'est **pas** un compte : c'est la clé du coffre local, pas une identité.

Un test de garde scanne les migrations et échoue si un identifiant interdit apparaît (cf. PLAN P0).

## 7. Canal app ↔ extension (loopback)

- L'app Tauri expose un **serveur HTTP/WS local sur `127.0.0.1`** (port choisi/annoncé), qui
  **n'écoute jamais hors loopback**.
- **Appairage** : à la première connexion, l'extension obtient un **token d'appairage** (capability)
  validé par une action explicite côté app. Le token est une **capacité d'accès au canal**, pas un
  secret de coffre ; il est le seul élément que l'extension a le droit de persister (cf. THREAT F14).
- Toute requête de l'extension est authentifiée par ce token + vérification d'origine. Le canal ne
  sert des identifiants que si **le coffre est déverrouillé**.
- Détail du protocole et du durcissement : THREAT_MODEL F7 + PLAN P6.

## 8. Design system

On réutilise le design **Studio** de `freelance-savior` (cf. son `design-mockup.html` et son
`CLAUDE.md` §Design) : surfaces crème, encre, accent terracotta, polices Inter / Fraunces /
JetBrains Mono, cards `rounded-2xl shadow-card`. **Tout le texte UI en français**, identifiants
techniques et commentaires en anglais.

## 9. Stack technique (résumé — voir CLAUDE.md pour le détail)

- **Shell** : Tauri 2 (Rust + webview, single binary, zéro cloud).
- **Front** : React 19 + TS strict + Vite + Tailwind v4, React Router, TanStack Query + Zustand,
  React Hook Form + Zod, lucide-react, sonner.
- **Backend Rust** : sqlx (SQLite + migrations), serde, uuid, chrono, thiserror, tokio.
- **Crypto** : **RustCrypto pur** — `argon2`, `chacha20poly1305`, `zeroize`, `rand` (CSPRNG OS).
  Aucune crypto maison, aucune dépendance C. Cf. CRYPTO_SPEC.
- **Extension** : MV3, `webextension-polyfill`.
- **Stockage** : SQLite dans l'app-data OS (`%APPDATA%\com.freepass.desktop\vault.sqlite` sur Windows).

## 10. Vision — accès agent IA scopé (futur, hors v1)

**Objectif** : pouvoir remettre à un **agent IA** (CLI, MCP, script, job CI) une **clé d'accès** qui
lui permet d'**utiliser certains mots de passe / secrets / variables d'environnement** du coffre, sans
que l'humain saisisse le mot de passe maître à chaque fois.

**Décisions de direction actées (2026-06-25)** :
- **Pas implémenté en v1** ; mais l'architecture v1 doit le rendre **additif**, pas migratoire.
- **Unité de scoping = l'environnement** : une clé d'accès donne droit à **un environnement** (ex.
  `projet-X-prod`), pas au coffre entier.
- **Cible = non-supervisé** : à terme, la clé d'accès doit pouvoir déchiffrer son environnement **sans
  mot de passe maître** (agent qui tourne en CI / la nuit). C'est pourquoi le modèle de clés est à
  **3 niveaux** dès v1 (`masterKey → vaultKey → envKey`, CRYPTO_SPEC §3) : donner accès à un agent =
  **emballer la `envKey` de l'environnement sous une clé dérivée de la clé d'accès** (CRYPTO_SPEC §9),
  sans re-chiffrer les entrées.

**Nouvel acteur** : **A6 — agent IA / sa clé d'accès compromise**. Entre dans le périmètre dès que la
fonctionnalité existe (THREAT_MODEL F16–F20). Exigences associées : **scoping strict** à un
environnement, **expiration**, **révocation effective** (= rotation de l'`envKey`, bornée à
l'environnement), **journal d'audit** des accès, **least privilege** (lecture seule par défaut).

**Surfaces possibles d'exposition** (à trancher à l'implémentation) : extension du canal loopback, un
**serveur MCP local**, ou une **CLI** qui injecte les variables d'un environnement. Aucun secret ne
sort de la machine sans décision figée séparée.

Détail crypto : CRYPTO_SPEC §9 (non figé). Phase de réalisation : PLAN Phase 11.
