import { Socket, Manager } from "socket.io-client";
import { DataSchemaEmitEvents, DataSchemaListenEvents } from "./IEvents";
import { SchemaMessage } from "./types/SchemaMessage";

export * from "./IEvents";

export type SocketIOClient = Socket<DataSchemaListenEvents, DataSchemaEmitEvents>;

export class StepDataSchemaBroadcastooor {
  private manager: Manager<DataSchemaListenEvents, DataSchemaEmitEvents>;
  private socket: SocketIOClient;
  constructor(url: string) {
    this.manager = new Manager<DataSchemaListenEvents, DataSchemaEmitEvents>(url);
    this.socket = this.manager.socket("/data_schema");
  }

  onConnect(listener: () => void | Promise<void>) {
    this.socket.on("connect", listener);
  }

  onDisconnect(listener: () => void | Promise<void>) {
    this.socket.on("disconnect", listener);
  }

  subscribe(topic: string, filterId?: string, filterExpression?: string) {
    if (!!filterId !== !!filterExpression) {
      throw new Error("filterId and filterExpression must both be set or both be undefined");
    } else if (filterId && filterExpression) {
      this.socket.emit("subscribe", { topic, "filter": { "id": filterId, "expression": filterExpression } });
    } else {
      this.socket.emit("subscribe", { topic });
    }
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