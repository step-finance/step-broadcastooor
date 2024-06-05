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

  onReceivedSchema(listener: (message: SchemaMessage) => void | Promise<void>): this {
    this.socket.on("receivedSchema", listener);
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
    this.manager.removeAllListeners();
    this.manager._close();
  }
}