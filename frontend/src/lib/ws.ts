import type { ClientMsg, ServerMsg } from "@/types/api";

export type WsStatus = "connecting" | "open" | "closed";

type MsgHandler = (msg: ServerMsg) => void;
type StatusHandler = (status: WsStatus) => void;

const MAX_RETRIES = 5;
const BASE_DELAY_MS = 1000;

export class WsClient {
  private ws: WebSocket | null = null;
  private retries = 0;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private msgHandlers = new Set<MsgHandler>();
  private statusHandlers = new Set<StatusHandler>();
  private status: WsStatus = "closed";

  connect(url: string) {
    this.cleanup();
    this.setStatus("connecting");
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.retries = 0;
      this.setStatus("open");
    };

    this.ws.onmessage = (event) => {
      try {
        const msg: ServerMsg = JSON.parse(event.data);
        this.msgHandlers.forEach((h) => h(msg));
      } catch (e) {
        console.error("invalid ws message", e);
      }
    };

    this.ws.onclose = () => {
      this.setStatus("closed");
      this.scheduleReconnect(url);
    };

    this.ws.onerror = (e) => {
      console.error("ws error", e);
    };
  }

  private scheduleReconnect(url: string) {
    if (this.retries >= MAX_RETRIES) {
      console.warn("ws max retries reached, stopping");
      return;
    }
    const delay = BASE_DELAY_MS * Math.pow(2, this.retries);
    this.retries += 1;
    this.retryTimer = setTimeout(() => this.connect(url), delay);
  }

  send(msg: ClientMsg) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  subscribe(handler: MsgHandler): () => void {
    this.msgHandlers.add(handler);
    return () => this.msgHandlers.delete(handler);
  }

  onStatus(handler: StatusHandler): () => void {
    this.statusHandlers.add(handler);
    handler(this.status);
    return () => this.statusHandlers.delete(handler);
  }

  private setStatus(status: WsStatus) {
    this.status = status;
    this.statusHandlers.forEach((h) => h(status));
  }

  private cleanup() {
    if (this.retryTimer) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws = null;
    }
  }

  disconnect() {
    this.retries = MAX_RETRIES;
    this.cleanup();
    this.setStatus("closed");
  }
}

export const wsClient = new WsClient();