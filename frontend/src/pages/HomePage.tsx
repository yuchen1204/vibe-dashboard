import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { getJson } from "@/lib/api";
import { wsClient } from "@/lib/ws";
import { useUiStore } from "@/stores/ui";
import type { HealthResponse, ServerMsg } from "@/types/api";

export function HomePage() {
  const { wsStatus, connectionId, pingPongLatency, setWsStatus, setConnectionId, setPingPongLatency } = useUiStore();
  const [pingSentAt, setPingSentAt] = useState<number | null>(null);

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: () => getJson<HealthResponse>("/api/health"),
    refetchInterval: 5000,
  });

  useEffect(() => {
    wsClient.connect(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`);

    const unsubStatus = wsClient.onStatus((status) => {
      setWsStatus(status);
      if (status === "open") {
        setPingSentAt(null);
        setPingPongLatency(0);
      }
    });

    const unsubMsg = wsClient.subscribe((msg: ServerMsg) => {
      if (msg.type === "hello") {
        setConnectionId(msg.payload.connection_id);
      } else if (msg.type === "pong") {
        if (pingSentAt) {
          setPingPongLatency(Date.now() - pingSentAt);
          setPingSentAt(null);
        }
      }
    });

    return () => {
      unsubStatus();
      unsubMsg();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handlePing = () => {
    setPingSentAt(Date.now());
    wsClient.send({ type: "ping" });
  };

  const wsStatusVariant = wsStatus === "open" ? "default" : wsStatus === "connecting" ? "secondary" : "destructive";

  return (
    <div className="flex min-h-screen">
      {/* sidebar placeholder moved to App.tsx layout */}
      <main className="flex-1 p-6">
        <h1 className="text-2xl font-bold mb-6">概览</h1>

        {wsStatus !== "open" && (
          <div className="mb-4 rounded-md border border-destructive bg-destructive/10 px-4 py-3 text-sm text-destructive">
            WebSocket 连接断开（{wsStatus}），正在尝试重连…
          </div>
        )}

        <div className="grid gap-4 md:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">后端健康</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm">
              {healthQuery.isLoading && <p className="text-muted-foreground">加载中…</p>}
              {healthQuery.isError && (
                <p className="text-destructive">后端不可达：{(healthQuery.error as Error).message}</p>
              )}
              {healthQuery.data && (
                <>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">状态</span>
                    <Badge variant="default">{healthQuery.data.status}</Badge>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">版本</span>
                    <span className="font-mono">{healthQuery.data.version}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">运行时长</span>
                    <span className="font-mono">{healthQuery.data.uptime_seconds.toFixed(1)}s</span>
                  </div>
                </>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-base">WebSocket 通道</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3 text-sm">
              <div className="flex justify-between items-center">
                <span className="text-muted-foreground">连接状态</span>
                <Badge variant={wsStatusVariant}>{wsStatus}</Badge>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">连接 ID</span>
                <span className="font-mono text-xs">
                  {connectionId ?? "-"}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Ping 延迟</span>
                <span className="font-mono">
                  {pingPongLatency != null ? `${pingPongLatency}ms` : "-"}
                </span>
              </div>
              <Button onClick={handlePing} disabled={wsStatus !== "open"} size="sm">
                发送 Ping
              </Button>
            </CardContent>
          </Card>
        </div>
      </main>
    </div>
  );
}
