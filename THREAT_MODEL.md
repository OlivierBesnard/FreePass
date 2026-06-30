# THREAT_MODEL.md — FreePass

> Failles **F1–F15** du modèle **coffre local** de FreePass, avec leur mitigation. C'est la grille de
> lecture du `security-reviewer` et la liste d'attaques du `tester`. Voir [`DESIGN.md`](DESIGN.md) §3
> (attaquants A1–A5) et [`CRYPTO_SPEC.md`](CRYPTO_SPEC.md) (contrat figé).
>
> ⚠️ FreePass n'est **pas** zero-knowledge multi-utilisateur : pas de serveur, pas de Modèle B, pas
> d'IDOR multi-tenant, pas de ≥2 admins, pas de récupération. Les menaces ZePass correspondantes
> **ne s'appliquent pas**. Ce modèle-ci est celui d'un **coffre chiffré au repos sur une seule machine**.

## Attaquants (rappel DESIGN §3)
- **A1** — vol du fichier coffre (disque, sauvegarde, backup OS).
- **A2** — site web de phishing tentant de capter un autofill.
- **A3** — application/malware local **non-root** parlant au canal loopback.
- **A4** — lecture passive (logs, messages d'erreur, presse-papier).
- **A5** — *(hors périmètre)* root/malware actif sur machine **déverrouillée**.
- **A6** — *(futur, dès que l'accès agent existe)* **agent IA / sa clé d'accès compromise** (F16–F20).

---

## Failles & mitigations

### 🔒 F1 — Coffre au repos lisible sans mot de passe maître
**Attaquant A1.** Si les champs sensibles étaient en clair dans SQLite, voler `vault.sqlite` suffirait.
**Mitigation** : `username`/`password`/`notes` **chiffrés AEAD** avec `vaultKey` (CRYPTO §4). Sans le
mdp maître, rien d'exploitable. `title`/`url` en clair = métadonnée assumée (cf. F5).

### 🔒 F2 — Mot de passe maître / clé persistés en clair
**A1/A4.** Une clé ou un mdp écrit sur disque (ou dans la config, le keychain non protégé, un cache)
casse tout.
**Mitigation** : `masterKey`/`vaultKey`/mdp maître **jamais persistés en clair** ; seuls `salt`,
`argon2_params`, `n0`, `wrappedVaultKey` (emballé) sont sur disque (CRYPTO §3). Clés en **mémoire
process uniquement**, dérivées à l'ouverture.

### 🔒 F3 — Secret résiduel en mémoire après lock
**A5-adjacent / dumps.** Une clé non effacée traîne après le verrouillage.
**Mitigation** : `zeroize`/`ZeroizeOnDrop` sur `masterKey`, `vaultKey`, plaintexts, mdp saisis/générés ;
effacement au lock / timeout / quit / chemins d'erreur (CRYPTO §6).

### 🔒 F4 — Brute-force du mot de passe maître (coffre volé)
**A1.** Attaque hors-ligne sur le fichier volé.
**Mitigation** : KDF **Argon2id** à coût mémoire élevé (≥ plancher §2 CRYPTO), sel aléatoire 16 o.
Le client **refuse** des paramètres sous le plancher (anti-affaiblissement). UX : exiger un mdp maître
fort (indicateur de force) à la création.

### 🔒 F5 — Fuite de secret dans logs / erreurs / IPC / oracle
**A4.** Un secret recraché dans un log, une erreur Tauri, une trace, ou un message qui distingue
« mauvais mdp » de « coffre corrompu » (oracle).
**Mitigation** : **aucun** clair de secret dans les logs/erreurs/`AppError` sérialisé vers le front.
Erreurs de déverrouillage **génériques** (CRYPTO §3.4). `url`/`title` en clair sont assumés non
secrets ; tout autre champ ne doit jamais apparaître en clair hors mémoire de travail.

### 🔒 F6 — Autofill sur le mauvais domaine (phishing)
**A2.** Un site `paypa1.com` obtient les identifiants de `paypal.com`.
**Mitigation** : **match d'origine strict** côté extension ET côté app — comparaison sur l'**origine
enregistrable** (eTLD+1) de l'onglet actif, jamais de remplissage cross-origin, jamais de remplissage
**silencieux** ; confirmation utilisateur avant injection. Pas de match par sous-chaîne d'URL.

### 🔒 F7 — Détournement du canal loopback par une autre app locale
**A3.** Un autre process sur `127.0.0.1` interroge le canal et siphonne le coffre.
**Mitigation** : serveur **loopback only** (n'écoute jamais hors 127.0.0.1) ; **token d'appairage**
obligatoire (validé par action explicite côté app à l'appairage) ; vérification d'**origine** des
requêtes (CORS strict, `Origin` de l'extension uniquement) ; le canal ne sert des secrets que si le
coffre est **déverrouillé**. Pas de port fixe devinable sans appairage.

### 🔒 F8 — Altération / rollback / swap d'entrée
**A1 (actif sur le fichier).** Remettre un vieux ciphertext, ou interchanger les `password` de deux
entrées.
**Mitigation** : **AEAD avec AAD** liant `entry_id` + `field_name` (CRYPTO §4). Tout swap/rollback ⇒
**échec MAC** au déchiffrement. Altérer 1 octet ⇒ rejet.

### 🔒 F9 — Fuite par le presse-papier
**A4.** Le mot de passe copié reste dans le presse-papier indéfiniment (et peut partir dans un
historique/synchro de presse-papier).
**Mitigation** : effacement **automatique** du presse-papier après un délai court (configurable) ;
ne pas logger la copie ; idéalement préférer l'autofill direct à la copie.

### 🔒 F10 — Réutilisation de nonce
**A1.** Deux chiffrements avec le même nonce sous la même clé cassent la confidentialité.
**Mitigation** : nonce **24 o aléatoire frais** par chiffrement via OsRng (CRYPTO §4-5) ; jamais de
nonce fixe hors vecteurs de test ; le `tester` cherche activement un nonce constant.

### 🔒 F11 — Verrouillage automatique absent
**A5-adjacent.** Coffre laissé ouvert indéfiniment sur un poste non surveillé.
**Mitigation** : **auto-lock** après inactivité (configurable), au verrouillage de session OS si
détectable, et à la fermeture ; le lock déclenche le `zeroize` (F3) et coupe le canal (F7).

### 🔒 F12 — Crypto maison / mauvaise primitive
**Mitigation** : **uniquement** les primitives de CRYPTO §1 (RustCrypto). Aucune implémentation
maison, aucun algo hors contrat. Revue systématique de tout `use` crypto.

### F13 — Export CSV en clair
**A1/A4.** L'export CSV contient les mots de passe en clair.
**Mitigation** : c'est **volontaire** (interop) mais doit être **explicitement confirmé** par
l'utilisateur avec avertissement ; pas d'export automatique ; documenter le risque. Symétriquement,
l'**import** CSV lit un fichier en clair fourni par l'utilisateur.

### 🔒 F14 — L'extension persiste un secret en clair
**A1 (profil navigateur volé).** Mettre une clé/un mdp dans `chrome.storage` les expose.
**Mitigation** : l'extension ne persiste **que le token d'appairage** (une **capability** d'accès au
canal, pas un secret de coffre) ; clés et identifiants déchiffrés vivent en **mémoire d'onglet/SW**
seulement, jamais en `localStorage`/`storage.local` durable. Le token seul ne déverrouille pas le coffre.

### 🔒 F15 — Mise à jour non signée (supply chain)
**A1/A3.** Un binaire ou une extension malveillante remplace FreePass.
**Mitigation** : builds de l'app **signés** + updater Tauri à signature **ed25519** (clé publique
épinglée) ; extension distribuée **signée par le store** (CWS/AMO). Pas d'auto-update non vérifié.

---

---

## Failles futures — accès agent IA (F16–F20, hors v1)

> N'entrent dans le périmètre **que** lorsque la fonctionnalité d'accès agent (DESIGN §10,
> CRYPTO_SPEC §9, PLAN Phase 11) est implémentée. Listées dès maintenant pour que l'architecture v1
> ne se ferme aucune porte. Attaquant **A6**.

### F16 — Clé d'accès agent compromise / exfiltrée
**A6.** La `accessSecret` détenue par l'agent fuit (dépôt, env CI, log de l'agent). Quiconque l'a peut
lire l'environnement scopé **sans mot de passe maître**.
**Mitigation** : portée **limitée à un environnement** (jamais le coffre entier) ; **expiration**
courte ; **révocation** = rotation de l'`envKey` (CRYPTO §9) ; jamais re-stockée en clair côté coffre ;
custody = responsabilité de l'intégration agent ; recommander lecture seule.

### F17 — Scope trop large / élévation
**A6.** Une clé d'accès donne plus que l'environnement prévu (ou un agent atteint un autre environnement).
**Mitigation** : un `grant` n'emballe **que** l'`envKey` de **son** environnement (CRYPTO §9) ; l'AAD
des entrées lie `env_id` (F8) ⇒ une `envKey` ne déchiffre pas un autre environnement.

### F18 — Révocation inefficace
**A6.** On « révoque » une clé mais l'agent a déjà lu l'`envKey` et continue d'accéder.
**Mitigation** : la révocation **doit rotationner** l'`envKey` (re-chiffrement borné à l'environnement),
pas seulement supprimer le `grant`. Documenté comme invariant (CRYPTO §9).

### F19 — Absence d'audit
**A6/A4.** Impossible de savoir quel agent a lu quoi et quand.
**Mitigation** : **journal d'audit** de chaque service de secret via une clé d'accès (quoi, quand,
quelle `access_id`) ; le journal ne contient **aucune valeur de secret**.

### F20 — Accès non-supervisé sur machine non fiable / exfiltration réseau
**A6.** L'agent tourne sans humain et pourrait renvoyer les secrets hors machine.
**Mitigation** : surface de service **locale** (loopback / MCP local / CLP) explicitement choisie et
figée ; aucun chemin réseau sortant pour un secret sans décision séparée ; least privilege + expiration.

---

## Cartographie attaquant → failles
| Attaquant | Failles à couvrir |
|-----------|-------------------|
| A1 vol du coffre | F1, F2, F4, F8, F13, F15 |
| A2 phishing | F6 |
| A3 app locale | F7, F14, F15 |
| A4 lecture passive | F5, F9, F13 |
| A5 *(hors périmètre)* | F3, F11 réduisent l'exposition mais ne couvrent pas A5 |
| A6 *(futur, accès agent)* | F16, F17, F18, F19, F20 |
