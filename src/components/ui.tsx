import type { ButtonHTMLAttributes, InputHTMLAttributes } from "react";

/** Shared input styling (Studio design system). */
export const inputClass =
  "h-10 w-full rounded-lg border border-cream-400 bg-card px-3 text-[14px] " +
  "text-ink-700 shadow-sm outline-none transition-colors placeholder:text-ink-400 " +
  "focus-visible:border-brand-400 focus-visible:ring-2 focus-visible:ring-brand-400/40";

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  const { className = "", ...rest } = props;
  return <input className={`${inputClass} ${className}`} {...rest} />;
}

export function Button(props: ButtonHTMLAttributes<HTMLButtonElement>) {
  const { className = "", ...rest } = props;
  return (
    <button
      className={
        "inline-flex h-10 items-center justify-center rounded-lg bg-brand-500 px-4 " +
        "text-sm font-medium text-white shadow-sm transition-colors hover:bg-brand-600 " +
        "disabled:cursor-not-allowed disabled:opacity-50 " +
        className
      }
      {...rest}
    />
  );
}

/** Centered card on the warm ambient background — shared by the auth screens. */
export function AuthShell({ children }: { children: React.ReactNode }) {
  return (
    <main className="bg-mesh flex min-h-full items-center justify-center p-8">
      <div className="anim-fade-in w-full max-w-md rounded-2xl border bg-card p-8 shadow-card">
        {children}
      </div>
    </main>
  );
}
