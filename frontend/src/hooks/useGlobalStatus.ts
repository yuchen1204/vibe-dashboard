import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { getJson } from "@/lib/api";
import { wsClient } from "@/lib/ws";
import { useUiStore } from "@/stores/ui";
import { useExecutionStore } from "@/stores/execution";
import type { HealthResponse, ServerMsg } from "@/types/api";

export function useGlobalStatus() {
  const { setWsStatus, setConnectionId, setPingPongLatency } = useUiStore();
  const setJobStatus = useExecutionStore((s) => s.setJobStatus);
  const pingSentAtRef = useRef<number | null>(null);

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: () => getJson<HealthResponse>("/api/health"),
    refetchInterval: 5000,
  });

  useEffect(() => {
    wsClient.connect(
      `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`,
    );

    const unsubStatus = wsClient.onStatus((status) => {
      setWsStatus(status);
      if (status === "open") {
        pingSentAtRef.current = null;
        setPingPongLatency(0);
      }
    });

    const unsubMsg = wsClient.subscribe((msg: ServerMsg) => {
      if (msg.type === "hello") {
        setConnectionId(msg.payload.connection_id);
      } else if (msg.type === "pong") {
        if (pingSentAtRef.current != null) {
          setPingPongLatency(Date.now() - pingSentAtRef.current);
          pingSentAtRef.current = null;
        }
      } else if (msg.type === "job_status") {
        setJobStatus(msg.payload.job_id, msg.payload.todo_id, msg.payload.status);
      }
    });

    return () => {
      unsubStatus();
      unsubMsg();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    healthOk: healthQuery.data?.status === "ok",
  };
}