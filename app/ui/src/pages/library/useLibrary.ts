import { useCallback, useEffect, useState } from "react";
import { api } from "@/shared/api/commands";
import type { GameSummary } from "@/shared/api/bindings/GameSummary";

export function useLibrary() {
  const [games, setGames] = useState<GameSummary[] | null>(null);
  const reload = useCallback(async () => {
    setGames(await api.listGames());
  }, []);
  useEffect(() => {
    void reload();
  }, [reload]);
  return { games, reload };
}
