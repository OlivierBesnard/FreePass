import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, type VaultStatus } from "./lib/api";
import { CreateVault } from "./screens/CreateVault";
import { UnlockVault } from "./screens/UnlockVault";
import { VaultHome } from "./screens/VaultHome";

/**
 * Root state machine for Phase 2. Reads the vault status and shows the right
 * screen: create vault (first run) → unlock (locked) → home (unlocked).
 * Router + TanStack Query come in Phase 3 with real navigation and data.
 */
function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.vaultStatus());
    } catch (err) {
      setLoadError(errorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  if (loadError) {
    return (
      <main className="bg-mesh flex min-h-full items-center justify-center p-8">
        <p className="text-sm text-danger-600">{loadError}</p>
      </main>
    );
  }

  if (status === null) {
    return (
      <main className="bg-mesh flex min-h-full items-center justify-center p-8">
        <p className="text-sm text-ink-500">Chargement…</p>
      </main>
    );
  }

  if (!status.initialized) {
    return <CreateVault onCreated={refresh} />;
  }
  if (!status.unlocked) {
    return <UnlockVault onUnlocked={refresh} />;
  }
  return <VaultHome onLock={refresh} />;
}

export default App;
