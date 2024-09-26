import { Socket, io } from "socket.io-client";
import { DataDoootEmitEvents, DataDoootListenEvents } from "./IEvents";
import { DoootMessage } from "./types/DoootMessage";

export * from "./IEvents";
export * from "./AllTypes";

export type SocketIOClient = Socket<DataDoootListenEvents, DataDoootEmitEvents>;

export class StepDataDoootBroadcastooor {
  private socket: SocketIOClient;
  constructor(url: string, token: string | undefined = undefined) {
    this.socket = io(url + "/dooots", {auth:{token}});
  }

  onConnect(listener: () => void | Promise<void>): this {
    this.socket.on("connect", listener);
    return this;
  }

  onDisconnect(listener: () => void | Promise<void>): this {
    this.socket.on("disconnect", listener);
    return this;
  }

  subscribe(topic: string, filterId?: string, filterExpression?: string): this {
    if (!!filterId !== !!filterExpression) {
      throw new Error("filterId and filterExpression must both be set or both be undefined");
    } else if (filterId && filterExpression) {
      this.socket.emit("subscribe", { topic, "filter": { "id": filterId, "expression": filterExpression } });
    } else {
      this.socket.emit("subscribe", { topic });
    }
    return this;
  }

  unsubscribe(topic: string, filterId?: string): this {
    this.socket.emit("unsubscribe", { topic, filter_id: filterId });
    return this;
  }

  onReceivedDooot(listener: (message: DoootMessage) => void | Promise<void>): this {
    this.socket.on("receivedDooot", listener);
    return this;
  }

  onSubscribed(listener: (topic: string) => void | Promise<void>): this {
    this.socket.on("subscribed", listener);
    return this;
  }

  onUnsubscribed(listener: (topic: string) => void | Promise<void>): this {
    this.socket.on("unsubscribed", listener);
    return this;
  }

  onServerError(listener: (error: string) => void | Promise<void>): this {
    this.socket.on("serverError", listener);
    return this;
  }

  close() {
    this.socket.removeAllListeners();
    this.socket.disconnect();
  }
}