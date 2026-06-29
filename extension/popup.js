"use strict";

// FreePass extension popup. Talks to the app's loopback channel on a FIXED port
// (auto-discovered among a few candidates), so the user never enters a port.
// It persists ONLY the pairing token (a capability, not a vault secret — F14),
// which is stable across app restarts. Credentials are fetched per-tab, shown
// for the current site only, and filled only on click (never silent — F6).

const CANDIDATE_PORTS = [48100, 48101, 48102];

const root = document.getElementById("root");

function el(tag, props = {}, children = []) {
  const node = document.createElement(tag);
  Object.assign(node, props);
  for (const c of [].concat(children)) {
    node.append(c instanceof Node ? c : document.createTextNode(c));
  }
  return node;
}

function clear() {
  root.replaceChildren();
}

function message(text) {
  clear();
  root.append(el("p", { className: "muted", textContent: text }));
}

async function getToken() {
  const { token } = await chrome.storage.local.get(["token"]);
  return token;
}

async function getActiveTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  return tab;
}

// Find the port the app is listening on (it's one of CANDIDATE_PORTS).
async function discoverPort() {
  for (const port of CANDIDATE_PORTS) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/health`, { method: "GET" });
      if (res.ok) return port;
    } catch {
      // try next
    }
  }
  return null;
}

async function fetchCredentials(token, origin) {
  const port = await discoverPort();
  if (port === null) throw "no-app";
  const res = await fetch(`http://127.0.0.1:${port}/credentials`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ origin }),
  });
  if (!res.ok) throw res.status;
  return res.json();
}

// Runs in the page (injected). Self-contained: no outer references.
function fillForm(username, password) {
  function setValue(elm, value) {
    const proto =
      elm.tagName === "TEXTAREA"
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(proto, "value").set;
    setter.call(elm, value);
    elm.dispatchEvent(new Event("input", { bubbles: true }));
    elm.dispatchEvent(new Event("change", { bubbles: true }));
  }
  const pw = document.querySelector('input[type="password"]');
  if (pw && password) setValue(pw, password);
  if (username) {
    let user = document.querySelector(
      'input[autocomplete="username"], input[type="email"], input[name*="user" i], input[name*="email" i], input[id*="user" i]',
    );
    if (!user) {
      const texts = Array.from(
        document.querySelectorAll('input[type="text"], input:not([type])'),
      );
      user = texts[0] || null;
    }
    if (user) setValue(user, username);
  }
  return true;
}

async function fill(cred) {
  const tab = await getActiveTab();
  if (!tab || tab.id == null) return;
  await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: fillForm,
    args: [cred.username || "", cred.password || ""],
  });
  window.close();
}

function renderCredentials(creds) {
  clear();
  if (creds.length === 0) {
    root.append(
      el("p", { className: "muted", textContent: "Aucun identifiant pour ce site." }),
    );
    return;
  }
  for (const cred of creds) {
    const meta = el("div", { className: "meta" }, [
      el("div", { className: "title", textContent: cred.title }),
      el("div", { className: "user", textContent: cred.username || "—" }),
    ]);
    const btn = el("button", { className: "fill", textContent: "Remplir" });
    btn.addEventListener("click", () => fill(cred));
    root.append(el("div", { className: "cred" }, [meta, btn]));
  }
}

function renderPairing(notice) {
  clear();
  if (notice) root.append(el("p", { className: "muted", textContent: notice }));
  root.append(el("label", { textContent: "Token d'appairage" }));
  const tokenInput = el("input", { id: "token", className: "mono", placeholder: "collez le token depuis FreePass" });
  root.append(tokenInput);
  const save = el("button", { className: "primary", textContent: "Appairer" });
  save.addEventListener("click", async () => {
    const token = tokenInput.value.trim();
    if (!token) return;
    await chrome.storage.local.set({ token });
    void init();
  });
  root.append(save);
  root.append(
    el("p", {
      className: "hint",
      textContent:
        "Dans FreePass : déverrouillez le coffre → « Connecter l'extension » → copiez le token. Une seule fois : il reste valable après redémarrage.",
    }),
  );
}

async function init() {
  try {
    const token = await getToken();
    if (!token) return renderPairing();

    const tab = await getActiveTab();
    const url = tab && tab.url ? tab.url : "";
    if (!/^https?:\/\//.test(url)) {
      return message("Ouvrez un site web pour voir vos identifiants.");
    }

    try {
      const creds = await fetchCredentials(token, url);
      renderCredentials(creds);
      const reset = el("button", { className: "link", textContent: "Ré-appairer" });
      reset.addEventListener("click", () => renderPairing());
      root.append(reset);
    } catch (status) {
      if (status === "no-app") {
        message("FreePass est-il ouvert et déverrouillé ?");
      } else if (status === 403) {
        message("Coffre verrouillé. Déverrouillez l'application FreePass.");
      } else if (status === 401) {
        renderPairing("Token invalide — re-collez le token affiché dans FreePass.");
      } else {
        message("Coffre injoignable.");
      }
    }
  } catch (e) {
    message("Erreur inattendue.");
  }
}

void init();
