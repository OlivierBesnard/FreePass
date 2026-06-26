import { useRef, useState } from "react";
import * as Papa from "papaparse";
import { AlertTriangle, FileUp } from "lucide-react";
import { toast } from "sonner";
import type { EntryInput } from "../lib/api";
import { useImportEntries } from "../hooks/useVault";
import { Modal } from "./Modal";
import { Button } from "./ui";

type Row = Record<string, string>;

function pick(row: Row, keys: string[]): string | null {
  for (const k of keys) {
    const v = row[k];
    if (v && v.trim().length > 0) return v.trim();
  }
  return null;
}

function hostFromUrl(url: string): string {
  try {
    return new URL(url.startsWith("http") ? url : `https://${url}`).hostname;
  } catch {
    return url;
  }
}

/** Map one CSV row (Chrome/Firefox/Bitwarden-ish exports) to an entry. */
function mapRow(raw: Row): EntryInput | null {
  const row: Row = {};
  for (const [k, v] of Object.entries(raw)) row[k.trim().toLowerCase()] = v ?? "";

  const url = pick(row, ["url", "website", "login_uri", "uri"]);
  const title = pick(row, ["name", "title"]) ?? (url ? hostFromUrl(url) : null);
  if (!title) return null;

  return {
    title,
    url,
    username: pick(row, ["username", "login", "login_username", "user", "email"]),
    password: pick(row, ["password", "login_password", "pass"]),
    notes: pick(row, ["note", "notes", "comment", "comments"]),
  };
}

/** CSV import flow: choose file → parse → preview + warning → confirm. */
export function ImportCsv({
  envId,
  onClose,
}: {
  envId: string;
  onClose: () => void;
}) {
  const fileInput = useRef<HTMLInputElement>(null);
  const [parsed, setParsed] = useState<EntryInput[] | null>(null);
  const [fileName, setFileName] = useState("");
  const importMutation = useImportEntries(envId);

  async function handleFile(file: File) {
    setFileName(file.name);
    const text = await file.text();
    const result = Papa.parse<Row>(text, {
      header: true,
      skipEmptyLines: true,
    });
    if (result.errors.length > 0) {
      toast.error("Le fichier CSV n'a pas pu être lu.");
      return;
    }
    const entries = result.data
      .map(mapRow)
      .filter((e): e is EntryInput => e !== null);
    if (entries.length === 0) {
      toast.error("Aucun identifiant exploitable dans ce fichier.");
      return;
    }
    setParsed(entries);
  }

  async function confirmImport() {
    if (!parsed) return;
    await importMutation.mutateAsync(parsed);
    onClose();
  }

  return (
    <Modal title="Importer un CSV" onClose={onClose}>
      {parsed === null ? (
        <div className="space-y-4">
          <p className="text-sm text-ink-600">
            Importez un export de mots de passe (Chrome, Firefox, Bitwarden…).
            Les colonnes <code className="kbd">name</code>,{" "}
            <code className="kbd">url</code>,{" "}
            <code className="kbd">username</code>,{" "}
            <code className="kbd">password</code> sont reconnues automatiquement.
          </p>
          <input
            ref={fileInput}
            type="file"
            accept=".csv,text/csv"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) void handleFile(f);
            }}
          />
          <button
            onClick={() => fileInput.current?.click()}
            className="flex w-full flex-col items-center gap-2 rounded-xl border border-dashed border-cream-500 bg-cream-200/50 px-4 py-8 text-ink-500 transition-colors hover:border-brand-300 hover:text-brand-700"
          >
            <FileUp size={26} />
            <span className="text-sm font-medium">Choisir un fichier .csv</span>
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          <p className="text-sm text-ink-700">
            <strong>{parsed.length}</strong> identifiant
            {parsed.length > 1 ? "s" : ""} prêt
            {parsed.length > 1 ? "s" : ""} à importer depuis{" "}
            <span className="font-mono text-ink-500">{fileName}</span>.
          </p>
          <div className="flex gap-2 rounded-lg border border-warning-200 bg-warning-50 p-3 text-xs text-warning-700">
            <AlertTriangle size={16} className="mt-0.5 shrink-0" />
            <p>
              Votre fichier CSV contient les mots de passe <strong>en clair</strong>.
              Après l'import, supprimez-le de façon sécurisée. FreePass les chiffre
              dans le coffre.
            </p>
          </div>
          <div className="flex justify-end gap-2">
            <button
              type="button"
              onClick={() => setParsed(null)}
              className="inline-flex h-10 items-center rounded-lg border border-cream-400 px-4 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
            >
              Choisir un autre fichier
            </button>
            <Button onClick={confirmImport} disabled={importMutation.isPending}>
              {importMutation.isPending ? "Import…" : "Importer"}
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
