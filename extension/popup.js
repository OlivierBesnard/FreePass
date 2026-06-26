"use strict";

// FreePass extension popup. Talks to the app's loopback channel (127.0.0.1) with
// the paired token. It persists ONLY the pairing token + port (a capability, not
// a vault secret — THREAT F14). Credentials are fetched per-tab, shown for the
// current site only, and filled only when the user clicks (never silent — F6).

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

async function getConfig() {
  return chrome.storage.local.get(["port", "token"]);
}

async function getActiveTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  return tab;
}

async function fetchCredentials(cfg, origin) {
  const res = await fetch(`http://127.0.0.1:${cfg.port}/credentials`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${cfg.token}`,
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
  root.append(el("label", { textContent: "Port" }));
  const portInput = el("input", { id: "port", type: "number", placeholder: "ex. 51234" });
  root.append(portInput);
  root.append(el("label", { textContent: "Token d'appairage" }));
  const tokenInput = el("input", { id: "token", className: "mono", placeholder: "collez le token" });
  root.append(tokenInput);
  const save = el("button", { className: "primary", textContent: "Appairer" });
  save.addEventListener("click", async () => {
    const port = Number(portInput.value);
    const token = tokenInput.value.trim();
    if (!port || !token) return;
    await chrome.storage.local.set({ port, token });
    void init();
  });
  root.append(save);
  root.append(
    el("p", {
      className: "hint",
      textContent:
        "Ouvrez FreePass, déverrouillez le coffre, puis « Connecter l'extension » pour copier ces valeurs.",
    }),
  );
}

async function init() {
  try {
    const cfg = await getConfig();
    if (!cfg.port || !cfg.token) return renderPairing();

    const tab = await getActiveTab();
    const url = tab && tab.url ? tab.url : "";
    if (!/^https?:\/\//.test(url)) {
      return message("Ouvrez un site web pour voir vos identifiants.");
    }

    try {
      const creds = await fetchCredentials(cfg, url);
      renderCredentials(creds);
      const reset = el("button", { className: "link", textContent: "Ré-appairer" });
      reset.addEventListener("click", () => renderPairing());
      root.append(reset);
    } catch (status) {
      if (status === 403) {
        message("Coffre verrouillé. Déverrouillez l'application FreePass.");
      } else if (status === 401) {
        renderPairing("Token invalide — ré-appairez avec les valeurs actuelles.");
      } else {
        message("Coffre injoignable. L'application FreePass est-elle ouverte ?");
      }
    }
  } catch (e) {
    message("Erreur inattendue.");
  }
}

void init();
