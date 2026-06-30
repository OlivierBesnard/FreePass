import { useCallback, useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  api,
  errorMessage,
  type ProjectInfo,
  type VaultStatus,
} from "./lib/api";
import { CreateVault } from "./screens/CreateVault";
import { UnlockVault } from "./screens/UnlockVault";
import { ProjectsHome } from "./screens/ProjectsHome";
import { VaultHome } from "./screens/VaultHome";
import { UpdateBanner } from "./components/UpdateBanner";

/**
 * Root state machine. Reads the vault status and shows the right screen:
 * create vault (first run) → unlock (locked) → projects → environment+entries.
 */
function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // Selected project (Phase 10). Kept in volatile UI state only.
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const queryClient = useQueryClient();

  const refresh = useCallback(async () => {
    try {
      const next = await api.vaultStatus();
      setStatus(next);
      // On leaving the unlocked state (manual lock, auto-lock, timeout): drop the
      // selected project AND purge the query cache, so decrypted entries — clear
      // passwords included — don't linger in the JS heap until GC (THREAT F3).
      if (!next.unlocked) {
        setProject(null);
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
  } else if (project) {
    screen = (
      <VaultHome
        project={project}
        onLock={refresh}
        onBack={() => setProject(null)}
      />
    );
  } else {
    screen = <ProjectsHome onLock={refresh} onOpen={setProject} />;
  }

  return (
    <div className="flex h-full flex-col">
      <UpdateBanner />
      <div className="min-h-0 flex-1">{screen}</div>
    </div>
  );
}

export default App;
