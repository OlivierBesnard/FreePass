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

// fetch with a hard timeout so a third party squatting a candidate port without
// answering can't freeze the popup on "Chargement…" (B10).
async function fetchWithTimeout(url, opts = {}, timeoutMs = 1500) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    return await fetch(url, { ...opts, signal: ctrl.signal });
  } finally {
    clearTimeout(timer);
  }
}

// Find the port the app is listening on (one of CANDIDATE_PORTS). A port only
// counts as FreePass if /health returns our JSON `{"status":"ok",…}` — a generic
// 401 from an unrelated service must NOT be mistaken for a bad token (B10).
// Returns { status: "ok" | "auth" | "none", port }.
async function discoverPort(token) {
  let authPort = null; // a port that answered 401 to a valid-looking request
  for (const port of CANDIDATE_PORTS) {
    try {
      const res = await fetchWithTimeout(`http://127.0.0.1:${port}/health`, {
        method: "GET",
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json().catch(() => null);
        if (data && data.status === "ok") return { status: "ok", port };
      } else if (res.status === 401 && authPort === null) {
        authPort = port;
      }
    } catch {
      // timeout / connection refused → try the next port
    }
  }
  if (authPort !== null) return { status: "auth", port: authPort };
  return { status: "none", port: null };
}

async function fetchCredentials(token, origin) {
  const found = await discoverPort(token);
  if (found.status === "none") throw "no-app";
  if (found.status === "auth") throw 401; // right app, wrong/expired token
  const res = await fetchWithTimeout(
    `http://127.0.0.1:${found.port}/credentials`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ origin }),
    },
    3000,
  );
  if (!res.ok) throw res.status;
  return res.json();
}

// Runs in the page (injected). Self-contained: no outer references. Returns true
// iff at least one field was actually filled, so the popup can report failure
// instead of closing silently (B11).
function fillForm(username, password) {
  function isVisible(elm) {
    if (!elm) return false;
    const rect = elm.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) return false;
    const style = window.getComputedStyle(elm);
    return style.visibility !== "hidden" && style.display !== "none";
  }
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

  let filled = false;

  // Prefer a VISIBLE login password field over a hidden or "create new password"
  // field (sign-up forms), rather than blindly taking the first one (B11).
  const pwFields = Array.from(document.querySelectorAll('input[type="password"]'));
  const pw =
    pwFields.find((e) => isVisible(e) && e.autocomplete !== "new-password") ||
    pwFields.find((e) => isVisible(e)) ||
    pwFields[0] ||
    null;
  if (pw && password) {
    setValue(pw, password);
    filled = true;
  }

  if (username) {
    let user =
      document.querySelector('input[autocomplete="username"]') ||
      document.querySelector('input[type="email"]');
    // Otherwise a visible text/email field within the same form as the password.
    if (!user && pw) {
      const scope = pw.form || document;
      user =
        Array.from(
          scope.querySelectorAll(
            'input[type="text"], input[type="email"], input:not([type])',
          ),
        ).find(isVisible) || null;
    }
    if (!user) {
      user = document.querySelector(
        'input[name*="user" i], input[name*="email" i], input[id*="user" i]',
      );
    }
    if (user) {
      setValue(user, username);
      filled = true;
    }
  }
  return filled;
}

async function fill(cred) {
  const tab = await getActiveTab();
  if (!tab || tab.id == null) return;
  const results = await chrome.scripting.executeScript({
    target: { tabId: tab.id },
    func: fillForm,
    args: [cred.username || "", cred.password || ""],
  });
  const ok = results && results[0] && results[0].result === true;
  if (!ok) {
    message("Aucun champ de connexion trouvé sur cette page.");
    return;
  }
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
    // The icon is a self-contained data: URL served by the app — no network.
    const lead = cred.icon
      ? el("img", { className: "icon", src: cred.icon, alt: "" })
      : el("span", { className: "icon icon-fallback", textContent: "🔑" });
    root.append(el("div", { className: "cred" }, [lead, meta, btn]));
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
