import { create } from "zustand";
import type { WsStatus } from "@/lib/ws";

interface UiState {
  wsStatus: WsStatus;
  connectionId: string | null;
  pingPongLatency: number | null;
  setWsStatus: (status: WsStatus) => void;
  setConnectionId: (id: string | null) => void;
  setPingPongLatency: (ms: number) => void;
}

export const useUiStore = create<UiState>((set) => ({
  wsStatus: "closed",
  connectionId: null,
  pingPongLatency: null,
  setWsStatus: (wsStatus) => set({ wsStatus }),
  setConnectionId: (connectionId) => set({ connectionId }),
  setPingPongLatency: (pingPongLatency) => set({ pingPongLatency }),
}));