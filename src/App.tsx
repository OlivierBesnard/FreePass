import { useCallback, useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api, errorMessage, type VaultStatus } from "./lib/api";
import { CreateVault } from "./screens/CreateVault";
import { UnlockVault } from "./screens/UnlockVault";
import { VaultHome } from "./screens/VaultHome";
import { UpdateBanner } from "./components/UpdateBanner";

/**
 * Root state machine. Reads the vault status and shows the right screen:
 * create vault (first run) → unlock (locked) → unified entry list. Projects and
 * environments are a secondary surface inside the unified list, not a step.
 */
function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const refresh = useCallback(async () => {
    try {
      const next = await api.vaultStatus();
      setStatus(next);
      // On leaving the unlocked state (manual lock, auto-lock, timeout): purge
      // the query cache, so decrypted entries — clear passwords included — don't
      // linger in the JS heap until GC (THREAT F3).
      if (!next.unlocked) {
        queryClient.clear();
      }
    } catch (err) {
      setLoadError(errorMessage(err));
    }
  }, [queryClient]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  let screen;
  if (loadError) {
    screen = (
      <main className="bg-mesh flex min-h-full items-center justify-center p-8">
        <p className="text-sm text-danger-600">{loadError}</p>
      </main>
    );
  } else if (status === null) {
    screen = (
      <main className="bg-mesh flex min-h-full items-center justify-center p-8">
        <p className="text-sm text-ink-500">Chargement…</p>
      </main>
    );
  } else if (!status.initialized) {
    screen = <CreateVault onCreated={refresh} />;
  } else if (!status.unlocked) {
    screen = <UnlockVault onUnlocked={refresh} />;
  } else {
    screen = <VaultHome onLock={refresh} />;
  }

  return (
    <div className="flex h-full flex-col">
      <UpdateBanner />
      <div className="min-h-0 flex-1">{screen}</div>
    </div>
  );
}

export default App;
