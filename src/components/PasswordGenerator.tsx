import { useState } from "react";
import { RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { api, errorMessage, type GeneratorOptions } from "../lib/api";

type Classes = Omit<GeneratorOptions, "length">;

const CLASS_LABELS: { key: keyof Classes; label: string }[] = [
  { key: "lowercase", label: "a-z" },
  { key: "uppercase", label: "A-Z" },
  { key: "digits", label: "0-9" },
  { key: "symbols", label: "!@#" },
];

/** Inline generator: length + character classes, calls the Rust CSPRNG command. */
export function PasswordGenerator({
  onGenerated,
}: {
  onGenerated: (password: string) => void;
}) {
  const [length, setLength] = useState(20);
  const [classes, setClasses] = useState<Classes>({
    lowercase: true,
    uppercase: true,
    digits: true,
    symbols: true,
  });

  function toggle(key: keyof Classes) {
    setClasses((c) => ({ ...c, [key]: !c[key] }));
  }

  async function generate() {
    try {
      const pw = await api.generatePassword({ length, ...classes });
      onGenerated(pw);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  }

  return (
    <div className="mt-2 rounded-lg border border-cream-400 bg-cream-200/60 p-3">
      <div className="flex items-center justify-between gap-3">
        <label className="flex flex-1 items-center gap-2 text-xs text-ink-600">
          Longueur
          <input
            type="range"
            min={6}
            max={64}
            value={length}
            onChange={(e) => setLength(Number(e.target.value))}
            className="flex-1 accent-brand-500"
          />
          <span className="w-6 font-mono text-ink-700">{length}</span>
        </label>
        <button
          type="button"
          onClick={generate}
          className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-brand-500 px-3 text-xs font-medium text-white transition-colors hover:bg-brand-600"
        >
          <RefreshCw size={13} /> Générer
        </button>
      </div>
      <div className="mt-2 flex gap-1.5">
        {CLASS_LABELS.map(({ key, label }) => (
          <button
            key={key}
            type="button"
            onClick={() => toggle(key)}
            className={`rounded-md px-2 py-1 font-mono text-xs transition-colors ${
              classes[key]
                ? "bg-brand-100 text-brand-700"
                : "bg-cream-300 text-ink-400"
            }`}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
