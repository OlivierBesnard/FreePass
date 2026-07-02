/**
 * Registrable-domain (eTLD+1) computation for the front-end, mirroring the Rust
 * `registrable_domain` / `host_of` logic in `local_channel.rs`. Used purely for
 * COSMETIC grouping of the unified entry list by site — never for any security
 * decision (autofill matching stays in Rust). Kept in sync with the Rust
 * `MULTI_SUFFIXES` list so the UI groups the same way the channel matches.
 */

/** Same minimal multi-label public suffixes as the Rust side. */
const MULTI_SUFFIXES = [
  "co.uk",
  "org.uk",
  "gov.uk",
  "ac.uk",
  "co.jp",
  "com.au",
  "net.au",
  "com.br",
  "co.nz",
  "co.za",
  "com.mx",
  "co.in",
] as const;

/** Extract the host from a URL or bare host string (drops scheme, path, port). */
function hostOf(input: string): string {
  let s = input.trim();
  // drop scheme
  const schemeSplit = s.split("://");
  s = schemeSplit.length > 1 ? schemeSplit[schemeSplit.length - 1] : s;
  s = s.split("/")[0] ?? s; // drop path
  s = s.split("?")[0] ?? s; // drop query
  s = s.split(":")[0] ?? s; // drop port
  return s.trim().replace(/\.+$/, "").toLowerCase();
}

/**
 * Registrable domain of a host (strips `www.`, honours the small multi-label
 * suffix list). Mirrors the Rust `registrable_domain`.
 */
function registrableFromHost(host: string): string {
  const stripped = host.startsWith("www.") ? host.slice(4) : host;
  const labels = stripped.split(".").filter((l) => l.length > 0);
  if (labels.length < 2) return stripped;
  const lastTwo = `${labels[labels.length - 2]}.${labels[labels.length - 1]}`;
  const take = (MULTI_SUFFIXES as readonly string[]).includes(lastTwo) ? 3 : 2;
  if (labels.length <= take) return labels.join(".");
  return labels.slice(labels.length - take).join(".");
}

/**
 * Registrable domain (eTLD+1) of a URL or host string, for grouping. Returns an
 * empty string when the input has no usable host (caller treats that as the
 * "no site" bucket).
 */
export function registrableDomain(url: string | null | undefined): string {
  if (!url) return "";
  const host = hostOf(url);
  if (!host) return "";
  return registrableFromHost(host);
}
