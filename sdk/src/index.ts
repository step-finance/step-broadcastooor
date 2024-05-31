import { io, Socket } from "socket.io-client";
import { DataSchemaEmitEvents, DataSchemaListenEvents } from "./IEvents";
import { SchemaMessage } from "./types";

export * from "./types";
export * from "./IEvents";
export type SocketIOClient = Socket<DataSchemaListenEvents, DataSchemaEmitEvents>;
export function createDataSchemaSocketIO(url: string): SocketIOClient {
  return io(url);
}
export class StepDataSchemaBroadcastooor {
  private socket: SocketIOClient;
  constructor(url: string) {
    this.socket = createDataSchemaSocketIO(url);
  }
  subscribe(topic: string) {
    this.socket.emit("subscribe", { topic });
  }
  unsubscribe(topic: string) {
    this.socket.emit("unsubscribe"), { topic };
  }
  onReceivedSchema(listener: (message: SchemaMessage) => void) {
    this.socket.on("receivedSchema", listener);
  }
  onSubscribed(listener: (topic: string) => void) {
    this.socket.on("subscribed", listener);
  }
  onUnsubscribed(listener: (topic: string) => void) {
    this.socket.on("unsubscribed", listener);
  }
  onServerError(listener: (error: string) => void) {
    this.socket.on("serverError", listener);
  }
  close() {
    this.socket.close();
  }
}