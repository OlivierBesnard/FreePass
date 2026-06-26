# Extension FreePass (MV3)

Pré-remplit les identifiants depuis le coffre **FreePass** local, via le canal
loopback (`127.0.0.1`) exposé par l'application de bureau.

## Modèle de sécurité
- Communique **uniquement** avec `http://127.0.0.1:<port>` (jamais le réseau).
- S'authentifie avec un **token d'appairage** (capability). Le token et le port
  sont les **seuls** éléments persistés (`storage.local`) — aucun secret de coffre
  n'est stocké (THREAT F14). Le token ne déverrouille pas le coffre.
- Le serveur ne répond qu'aux origines `*-extension://` (une page web ne peut pas
  lire les réponses) et **uniquement coffre déverrouillé**.
- Les identifiants ne sont proposés que pour le **domaine du site courant**
  (match strict côté app, THREAT F6) et ne sont remplis qu'après **un clic** —
  jamais de remplissage silencieux.

## Installation (dev, non empaquetée)

### Chrome / Edge
1. `chrome://extensions` → activez le **mode développeur**.
2. **Charger l'extension non empaquetée** → sélectionnez le dossier `extension/`.

### Firefox
1. `about:debugging#/runtime/this-firefox` → **Charger un module temporaire**.
2. Sélectionnez `extension/manifest.json`.

## Appairage
1. Ouvrez FreePass, **déverrouillez** le coffre.
2. Cliquez sur l'icône **puzzle « Connecter l'extension »** → copiez le **port** et
   le **token**.
3. Ouvrez le popup de l'extension → collez port + token → **Appairer**.
4. Sur un site, le popup liste les identifiants correspondants → **Remplir**.

> Le token change à chaque déverrouillage : ré-appairez si l'extension affiche
> « token invalide ».

## À venir (Phase 9)
- Icônes (reprise de l'icône de l'application).
- `webextension-polyfill` + empaquetage signé pour Chrome Web Store / AMO.
