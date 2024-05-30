import { io, Socket } from "socket.io-client";
import { DataSchemaEmitEvents, DataSchemaListenEvents } from "./IEvents";

export * from "./types";
export * from "./IEvents";
export type SocketIOClient = Socket<DataSchemaListenEvents, DataSchemaEmitEvents>;
export function createDataSchemaSocketIO(url: string): SocketIOClient {
  return io(url);
}