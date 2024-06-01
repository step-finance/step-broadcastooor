import { io, Socket, Manager } from "socket.io-client";
import { DataSchemaEmitEvents, DataSchemaListenEvents } from "./IEvents";
import { Filter, SchemaMessage } from "./types";

export * from "./types";
export * from "./IEvents";

export type SocketIOClient = Socket<DataSchemaListenEvents, DataSchemaEmitEvents>;

export class StepDataSchemaBroadcastooor {
  private manager: Manager<DataSchemaListenEvents, DataSchemaEmitEvents>;
  private socket: SocketIOClient;
  constructor(url: string) {
    this.manager = new Manager<DataSchemaListenEvents, DataSchemaEmitEvents>(url);
    this.socket = this.manager.socket("/data_schema");
  }

  subscribe(topic: string, filterId?: string, filterExpression?: string) {
    if (!!filterId !== !!filterExpression) {
      throw new Error("filterId and filterExpression must both be set or both be undefined");
    }
    this.socket.emit("subscribe", { topic, "filter": { "id": filterId, "expression": exp } });
  }
  
  unsubscribe(topic: string, filterId?: string) {
    this.socket.emit("unsubscribe", { topic, filter_id: filterId });
  }

  onReceivedSchema(listener: (message: SchemaMessage) => void | Promise<void>) {
    this.socket.on("receivedSchema", listener);
  }

  onSubscribed(listener: (topic: string) => void | Promise<void>) {
    this.socket.on("subscribed", listener);
  }

  onUnsubscribed(listener: (topic: string) => void | Promise<void>) {
    this.socket.on("unsubscribed", listener);
  }

  onServerError(listener: (error: string) => void | Promise<void>) {
    this.socket.on("serverError", listener);
  }

  close() {
    this.socket.removeAllListeners();
    this.socket.disconnect();
    this.manager.removeAllListeners();
    this.manager._close();
  }
}