/** Tiny master-password strength heuristic for the create-vault hint. */
export interface Strength {
  score: number; // 0..4
  label: string;
}

const LABELS = ["Très faible", "Faible", "Moyen", "Bon", "Fort"];

export function passwordStrength(pw: string): Strength {
  let s = 0;
  if (pw.length >= 8) s++;
  if (pw.length >= 12) s++;
  if (/[a-z]/.test(pw) && /[A-Z]/.test(pw)) s++;
  if (/\d/.test(pw)) s++;
  if (/[^A-Za-z0-9]/.test(pw)) s++;
  const score = Math.min(s, 4);
  return { score, label: LABELS[score] };
}
