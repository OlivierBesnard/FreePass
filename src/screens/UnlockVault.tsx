import { useState } from "react";
import { Lock } from "lucide-react";
import { api } from "../lib/api";
import { AuthShell, Button, Input } from "../components/ui";

// Generic message on purpose (anti-oracle, THREAT F5): we never tell the user
// whether the password was wrong or the vault was corrupted.
const GENERIC_ERROR = "Mot de passe incorrect ou coffre invalide.";

/** Returning-user screen: unlock the existing vault. */
export function UnlockVault({ onUnlocked }: { onUnlocked: () => void }) {
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (submitting || password.length === 0) return;
    setSubmitting(true);
    setError(null);
    try {
      await api.unlock(password);
      onUnlocked();
    } catch {
      setError(GENERIC_ERROR);
      setSubmitting(false);
    }
  }

  return (
    <AuthShell>
      <div className="mb-6 text-center">
        <div className="avatar-gradient mx-auto flex h-14 w-14 items-center justify-center rounded-2xl text-white shadow-pop">
          <Lock size={24} aria-hidden="true" />
        </div>
        <h1 className="mt-4 font-serif text-2xl font-semibold text-ink-800">
          Déverrouiller FreePass
        </h1>
        <p className="mt-2 text-sm text-ink-500">
          Saisissez votre mot de passe maître pour ouvrir le coffre.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-4">
        <Input
          type="password"
          autoFocus
          autoComplete="current-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="Mot de passe maître"
        />
        {error && <p className="text-sm text-danger-600">{error}</p>}
        <Button
          type="submit"
          className="w-full"
          disabled={submitting || password.length === 0}
        >
          {submitting ? "Déverrouillage…" : "Déverrouiller"}
        </Button>
      </form>
    </AuthShell>
  );
}
