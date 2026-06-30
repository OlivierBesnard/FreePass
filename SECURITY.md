# SECURITY.md — état des mitigations (Phase 8)

Passe de durcissement : où chaque faille de [`THREAT_MODEL.md`](THREAT_MODEL.md)
est traitée, avec un pointeur de code/test. ✅ implémenté · 🟡 partiel · ⏳ Phase 9.

| # | Faille | Statut | Mitigation & preuve |
|---|--------|:--:|---------------------|
| F1 | Coffre au repos lisible | ✅ | Champs chiffrés par AEAD sous l'`envKey` — `crypto/mod.rs`, table `entry_fields`. Test `entries::secret_fields_are_unreadable_on_disk`. |
| F2 | Clés/mdp persistés en clair | ✅ | Seuls `salt`, `params`, blobs emballés sur disque — `services/vault.rs`. Test `no_clear_key_or_password_material_on_disk`. |
| F3 | Secret résiduel en mémoire | ✅ | `SecretKey` = `ZeroizeOnDrop`, sans `Debug`/`Serialize` — `crypto/keys.rs`. `lock` libère la session ; `zeroize_password` sur l'IPC. |
| F4 | Brute-force mdp maître | ✅ | Argon2id (défaut 64 MiB) + **refus sous le plancher** — `crypto/kdf.rs`. Test `refuses_params_below_the_floor`. |
| F5 | Fuite logs/erreurs / oracle | ✅ | `AppError::Crypto` message **générique** unique ; déverrouillage indistinct. Test `unlock_with_wrong_password_is_a_generic_crypto_error`. |
| F6 | Autofill mauvais domaine | ✅ | Match **domaine enregistrable** strict côté app + extension limitée au site courant + remplissage **sur clic** — `services/local_channel.rs`. Tests `domains_match_is_strict`. 🟡 PSL complète = amélioration future (heuristique multi-suffixes en place). |
| F7 | Canal loopback détourné | ✅ | Bind `127.0.0.1` seul, **token Bearer**, garde d'origine + CORS `*-extension://`, service **déverrouillé uniquement**. Test live-socket `live_server_enforces_token_and_origin` (401/403/200). |
| F8 | Altération/rollback/swap | ✅ | AAD liant `env_id`+`entry_id`+`field_name` — `crypto/mod.rs`. Tests `field_does_not_decrypt_under_a_swapped_*`. |
| F9 | Fuite presse-papier | ✅ | Effacement auto après 20 s — `src/lib/clipboard.ts` (`copySecret`). |
| F10 | Réutilisation de nonce | ✅ | Nonce 24 o **frais OsRng** par chiffrement — `crypto/aead.rs`. |
| F11 | Verrouillage auto absent | ✅ | Auto-lock après 10 min d'inactivité — `src/hooks/useAutoLock.ts` ; lock au quit (drop de `AppState`). 🟡 détection verrouillage session OS = future. |
| F12 | Crypto maison | ✅ | RustCrypto uniquement (`argon2`, `chacha20poly1305`, `zeroize`, `rand`). |
| F13 | Export CSV en clair | 🟡 | Pas d'export en v1 ; l'**import** avertit que le CSV source est en clair — `src/components/ImportCsv.tsx`. À ré-évaluer si un export est ajouté. |
| F14 | Extension persiste un secret | ✅ | Seuls token + port en `storage.local` (capability) ; identifiants en mémoire de popup — `extension/popup.js`, `extension/README.md`. |
| F15 | Mise à jour non signée | ⏳ | Updater Tauri ed25519 + signature store = **Phase 9** (custody de clé humaine). Voir `RELEASING.md`. |

## Limites assumées (DESIGN §3, CRYPTO_SPEC §8)
- **A5** (root/malware sur machine **déverrouillée**) : hors périmètre — secrets en clair en mémoire une fois ouvert.
- **Aucune récupération** du mot de passe maître : perte = perte du coffre (sauvegarde du fichier = responsabilité utilisateur).
- **F16–F20** (accès agent IA) : non applicables tant que la Phase 11 n'est pas implémentée ; l'architecture (`envKey`) est prête.
