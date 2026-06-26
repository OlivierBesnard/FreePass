import { useState } from "react";
import { AlertTriangle, Compass } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { passwordStrength } from "../lib/password";
import { AuthShell, Button, Input } from "../components/ui";

const MIN_LENGTH = 8;
const BAR_COLORS = [
  "bg-danger-500",
  "bg-danger-500",
  "bg-warning-500",
  "bg-success-500",
  "bg-success-600",
];

/** First-run screen: choose the master password and create the vault. */
export function CreateVault({ onCreated }: { onCreated: () => void }) {
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const strength = passwordStrength(password);
  const tooShort = password.length > 0 && password.length < MIN_LENGTH;
  const mismatch = confirm.length > 0 && confirm !== password;
  const canSubmit =
    password.length >= MIN_LENGTH && password === confirm && !submitting;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setSubmitting(true);
    setError(null);
    try {
      await api.createVault(password);
      onCreated();
    } catch (err) {
      setError(errorMessage(err));
      setSubmitting(false);
    }
  }

  return (
    <AuthShell>
      <div className="mb-6 text-center">
        <div className="avatar-gradient mx-auto flex h-14 w-14 items-center justify-center rounded-2xl text-white shadow-pop">
          <Compass size={26} aria-hidden="true" />
        </div>
        <h1 className="mt-4 font-serif text-2xl font-semibold text-ink-800">
          Créer votre coffre
        </h1>
        <p className="mt-2 text-sm text-ink-500">
          Choisissez un mot de passe maître. Il déverrouille tous vos secrets et
          ne quitte jamais cette machine.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="mb-1.5 block text-sm font-medium text-ink-700">
            Mot de passe maître
          </label>
          <Input
            type="password"
            autoFocus
            autoComplete="new-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Au moins 8 caractères"
          />
          {password.length > 0 && (
            <div className="mt-2 flex items-center gap-2">
              <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-cream-300">
                <div
                  className={`h-full rounded-full transition-all ${BAR_COLORS[strength.score]}`}
                  style={{ width: `${((strength.score + 1) / 5) * 100}%` }}
                />
              </div>
              <span className="text-xs text-ink-500">{strength.label}</span>
            </div>
          )}
          {tooShort && (
            <p className="mt-1 text-xs text-danger-600">
              8 caractères minimum.
            </p>
          )}
        </div>

        <div>
          <label className="mb-1.5 block text-sm font-medium text-ink-700">
            Confirmer le mot de passe
          </label>
          <Input
            type="password"
            autoComplete="new-password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            placeholder="Saisissez-le à nouveau"
          />
          {mismatch && (
            <p className="mt-1 text-xs text-danger-600">
              Les mots de passe ne correspondent pas.
            </p>
          )}
        </div>

        <div className="flex gap-2 rounded-lg border border-warning-200 bg-warning-50 p-3 text-xs text-warning-700">
          <AlertTriangle size={16} className="mt-0.5 shrink-0" aria-hidden="true" />
          <p>
            <strong>Aucune récupération possible.</strong> Si vous oubliez ce mot
            de passe, le coffre est définitivement perdu. Pensez à sauvegarder le
            fichier du coffre.
          </p>
        </div>

        {error && <p className="text-sm text-danger-600">{error}</p>}

        <Button type="submit" className="w-full" disabled={!canSubmit}>
          {submitting ? "Création…" : "Créer le coffre"}
        </Button>
      </form>
    </AuthShell>
  );
}
