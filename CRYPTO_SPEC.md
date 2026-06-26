# CRYPTO_SPEC.md — FreePass — contrat crypto v1 (FIGÉ)

> Ce document est le **contrat crypto figé** de FreePass. Les agents l'**appliquent** ; ils ne le
> réinterprètent pas. Toute déviation est un défaut de sécurité. Si une tâche semble exiger un écart,
> **on s'arrête et on en discute** — on ne devine jamais sur la crypto.
>
> Périmètre : **coffre local chiffré au repos**, mono-utilisateur, **pas de E2E réseau**, **pas de
> récupération**. Voir [`DESIGN.md`](DESIGN.md) §3-4 et [`THREAT_MODEL.md`](THREAT_MODEL.md).

## 1. Primitives (RustCrypto pur — aucune autre)

| Usage | Algorithme | Crate |
|-------|-----------|-------|
| Dérivation du mdp maître (KDF) | **Argon2id** | `argon2` |
| Chiffrement authentifié (AEAD) | **XChaCha20-Poly1305** | `chacha20poly1305` (`XChaCha20Poly1305`) |
| Aléa | **CSPRNG OS** | `rand` / `getrandom` (`OsRng`) |
| Effacement mémoire | **zeroize** | `zeroize` (`Zeroize`, `ZeroizeOnDrop`) |

**Interdits** : crypto maison, MD5/SHA1, AES-ECB, `chacha20` sans Poly1305, RNG non cryptographique
(`rand::thread_rng` est OK car CSPRNG ; **jamais** un PRNG seedé manuellement), OpenSSL/ring en
parallèle, réutilisation de nonce, clé dérivée sans sel.

## 2. Paramètres Argon2id (plancher non négociable)

```
algorithm   = Argon2id
version      = 0x13 (v19)
memory (m)   = 65536 KiB  (64 MiB)     # plancher : 19456 KiB (19 MiB, OWASP) — ne JAMAIS descendre en dessous
iterations(t)= 3                        # plancher : 2
parallelism(p)= 1
salt          = 16 octets aléatoires (OsRng), unique par coffre
output        = 32 octets (clé maître dérivée)
```

- Le sel est **stocké en clair** dans l'enregistrement coffre (un sel n'est pas un secret).
- Les paramètres sont **stockés** avec le coffre pour permettre une future ré-augmentation. À
  l'ouverture, si les paramètres lus sont **sous le plancher**, le client **refuse** (anti-affaiblissement,
  cf. THREAT F4). Les paramètres ne viennent jamais d'une source réseau.

## 3. Hiérarchie de clés (3 niveaux)

On sépare trois niveaux : la **clé dérivée du mot de passe** (`masterKey`), la **clé racine du coffre**
(`vaultKey`), et **une clé par environnement** (`envKey`). Le niveau `envKey` n'est **pas** un luxe :
il rend l'**accès agent scopé futur** (cf. §9) ajoutable **sans re-chiffrer le coffre**, et il permet
de **changer le mot de passe maître sans toucher aux données**.

```
masterKey       = Argon2id(masterPassword, salt, params)         # 32 o, jamais persistée
vaultKey        = OsRng(32 o)                                     # générée à l'init, jamais persistée en clair
wrappedVaultKey = XChaCha20Poly1305_seal(
                      key  = masterKey,
                      nonce= n0 (24 o aléatoires),
                      data = vaultKey,
                      aad  = "freepass:v1:vaultkey")

# Par environnement (v1 ⇒ au moins un environnement par défaut) :
envKey[e]         = OsRng(32 o)                                   # générée à la création de l'environnement e
wrappedEnvKey[e]  = XChaCha20Poly1305_seal(
                      key  = vaultKey,
                      nonce= ne (24 o aléatoires),
                      data = envKey[e],
                      aad  = "freepass:v1:envkey:" || env_id[e])
```

Stockage :
- Table **`vault`** (une seule ligne) : `salt`, `argon2_params`, `n0`, `wrappedVaultKey`, `kdf_version`,
  `format_version`. Jamais `masterKey`/`vaultKey` en clair ni le mot de passe maître.
- Table **`environments`** (une ligne par environnement) : `env_id`, `name` (clair, métadonnée), `ne`,
  `wrappedEnvKey`. Jamais `envKey` en clair.

> **Pourquoi cette indirection en v1.** Chiffrer les entrées sous `envKey` (et non directement sous
> `vaultKey`) est la **seule** décision v1 nécessaire pour que l'accès agent non-supervisé (§9)
> devienne un **ajout** (un second emballage de `envKey`) plutôt qu'une **migration** (re-chiffrer
> tout). On l'adopte donc dès maintenant, même si v1 n'expose qu'un environnement par défaut.

### Déverrouillage
1. Lire `salt`, `params`, `n0`, `wrappedVaultKey`. Refuser si `params` < plancher (§2).
2. `masterKey = Argon2id(saisie, salt, params)`.
3. `vaultKey = XChaCha20Poly1305_open(masterKey, n0, wrappedVaultKey, aad="freepass:v1:vaultkey")`.
4. **Succès AEAD** ⇒ mot de passe correct, `vaultKey` en mémoire. **Échec AEAD** ⇒ erreur
   **générique** « coffre verrouillé / informations invalides » (anti-oracle, THREAT F5). Pas de
   vérificateur séparé : le déballage *est* la vérification.
5. Pour chaque environnement nécessaire : `envKey[e] = XChaCha20Poly1305_open(vaultKey, ne,
   wrappedEnvKey[e], aad="freepass:v1:envkey:"||env_id[e])` (à la demande ou à l'unlock).
6. `zeroize(masterKey)` et `zeroize(saisie)` dès que possible ; `envKey`/`vaultKey` zeroizées au lock.

### Changement de mot de passe maître
Re-dériver `masterKey'` depuis le nouveau mdp + **nouveau sel**, ré-emballer le **même** `vaultKey`
sous `masterKey'` avec un **nouveau nonce**. Les entrées ne sont pas touchées.

## 4. Chiffrement des champs d'entrée

Chaque champ secret d'une entrée est chiffré **indépendamment** avec la **`envKey` de l'environnement
auquel l'entrée appartient** (et non directement `vaultKey` — cf. §3) :

```
ciphertext_field = XChaCha20Poly1305_seal(
    key  = envKey[env_id],
    nonce= n (24 o aléatoires FRAIS, OsRng, par chiffrement),
    data = plaintext_utf8,
    aad  = "freepass:v1:entry:" || env_id || ":" || entry_id || ":" || field_name)
```

- **AAD obligatoire** liant `env_id` + `entry_id` + `field_name` ⇒ rejet automatique d'un **swap**
  d'entrée (y compris entre environnements) ou d'un **rollback** de champ (THREAT F8). Mauvais
  id/champ/env ⇒ échec MAC.
- **Nonce** : 24 octets, **aléatoire frais à chaque chiffrement** (jamais réutilisé, jamais fixe hors
  vecteurs de test). Stocké à côté du ciphertext (un nonce n'est pas secret).
- `field_name` selon le type d'entrée : `{ "username", "password", "notes" }` (login), `{ "value" }`
  (secret), ou la **clé de variable** (environnement, ex. `"API_KEY"`). `title`/`url`/`name` et le
  `name` de l'environnement restent en clair (métadonnées, DESIGN §4).
- Stockage par champ : `{ nonce, ciphertext }` (le tag Poly1305 est inclus dans `ciphertext`).

## 5. Aléa

- **Toute** valeur aléatoire (sel, clés, nonces, mdp générés) provient de l'**OsRng** (CSPRNG du
  système). Jamais `StdRng`/`SmallRng` seedés à la main pour du matériel cryptographique.
- Le **générateur de mots de passe** (feature) utilise aussi `OsRng` avec sélection **non biaisée**
  (rejet d'échantillonnage, pas de `% len`).

## 6. Gestion mémoire (zeroize)

- `masterKey`, `vaultKey`, plaintexts déchiffrés, mdp maître saisi, mdp générés : types portant
  `Zeroize`/`ZeroizeOnDrop`. Effacés au **lock**, au **timeout**, au **quit**, et sur tous les
  chemins d'erreur (`Drop`/`finally`).
- Côté React, minimiser la durée de vie des secrets en string state ; ne jamais les écrire en
  storage (cf. THREAT F2/F14).

## 7. Vecteurs de test (obligatoire)

Le module crypto Rust DOIT fournir des **vecteurs de test** déterministes (sel/nonce/clé fixés
**uniquement dans les tests**) couvrant : dérivation Argon2id, wrap/unwrap de `vaultKey`, seal/open
d'un champ avec AAD, **échec attendu** quand on altère 1 octet (ciphertext / nonce / AAD / entry_id).
Ils vivent dans `src-tauri/.../crypto/` (tests) et/ou `vectors/v1/`.

## 8. Ce que la crypto NE fait pas (limites assumées)

- **Pas de protection contre A5** (root/malware sur machine déverrouillée) : une fois le coffre
  ouvert, les secrets sont en mémoire claire. Documenté, hors périmètre.
- **Pas de récupération** : aucune copie de `vaultKey` n'est emballée ailleurs. Perte du mdp maître =
  perte définitive. (Décision de cadrage 2026-06-25.)
- **Pas de E2E réseau** : il n'y a pas de réseau. Le canal extension est **loopback only** et ne
  transporte des secrets que vers l'extension locale appairée, jamais hors machine (THREAT F7).

## 9. Extension future — clés d'accès agent IA (NON FIGÉ, hors v1)

> Cette section décrit la **cible** pour donner à un agent IA un accès **scopé à un environnement**,
> potentiellement **non-supervisé** (sans mot de passe maître). Elle n'est **pas** implémentée en v1
> et **n'est pas figée** ; seule l'**indirection `envKey` de §3 est adoptée dès v1** pour la rendre
> additive. À re-spécifier (et re-figer) au moment de l'implémentation. Voir DESIGN §10, THREAT F16–F20,
> PLAN Phase 10.

### Principe : un second emballage de `envKey`
Donner à un agent l'accès à l'environnement `e` = créer une **clé d'accès** et **emballer la même
`envKey[e]`** sous une clé dérivée de cette clé d'accès — **sans toucher** aux entrées (c'est tout
l'intérêt de l'indirection §3).

```
accessSecret   = OsRng(32 o)                 # remis à l'agent UNE fois (la "clé d'accès"), jamais re-stocké en clair côté coffre
accessKey      = HKDF-SHA256(accessSecret, info="freepass:v1:accesskey")   # ou Argon2id si l'access secret est faible
grant[e,a]     = XChaCha20Poly1305_seal(
                    key  = accessKey,
                    nonce= ng (24 o aléatoires),
                    data = envKey[e],
                    aad  = "freepass:v1:grant:" || env_id[e] || ":" || access_id[a])
```

- L'agent détenteur de `accessSecret` peut, **sans mot de passe maître**, recalculer `accessKey`,
  ouvrir `grant` → obtenir `envKey[e]` → déchiffrer **uniquement** l'environnement `e`. Non-supervisé OK.
- Le coffre stocke `grant[e,a]` + métadonnées de la clé d'accès (`access_id`, `name`, `env_id`,
  `expires_at`, `created_at`, `revoked_at`) — **jamais** `accessSecret` en clair (au plus un **hash**
  pour reconnaître/auditer, jamais de quoi rejouer).
- **Révocation** d'une clé d'accès = supprimer son `grant` **ET rotationner `envKey[e]`** (générer une
  nouvelle `envKey[e]`, re-chiffrer l'environnement `e` — borné à un seul environnement — et ré-emballer
  pour `vaultKey` + les clés d'accès encore valides). Sans rotation, un agent ayant déjà lu `envKey[e]`
  garde l'accès : la révocation **doit** rotationner.
- **Expiration** : `expires_at` vérifié à chaque service ; au-delà, refus + rotation recommandée.
- **Portée** : une clé d'accès = **un** environnement (granularité de scoping retenue, DESIGN §10).

### Contraintes de sécurité de l'extension agent (à figer plus tard)
- `accessSecret` est un **secret persistant détenu par l'agent** : surface d'attaque réelle (THREAT F16).
  Le coffre ne le re-stocke jamais en clair ; sa custody est la responsabilité de l'intégration agent.
- Tout service d'un secret via une clé d'accès est **journalisé** (audit : quoi, quand, quelle clé) —
  THREAT F19.
- Le canal de service reste **loopback** ou un transport local explicitement choisi ; aucun secret ne
  sort de la machine sans décision figée séparée.
