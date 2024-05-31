import { SchemaMessage, SubscribeRequest } from "./types";

export interface DataSchemaListenEvents {
  subscribed: (topic: string) => void;
  unsubscribed: (topic: string) => void;
  serverError: (error: string) => void;
  receivedSchema: (message: SchemaMessage) => void;
}

export interface DataSchemaEmitEvents {
  subscribe: (req: SubscribeRequest) => void;
  unsubscribe: () => void;
}