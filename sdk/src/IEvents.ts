import { SchemaMessage, SubscribeRequest } from "./types";

export interface DataSchemaListenEvents {
  subscribed: (topic: string) => void;
  unSubscribed: (topic: string) => void;
  serverError: (message: string) => void;
  receivedSchema: (schema: SchemaMessage) => void;
}

export interface DataSchemaEmitEvents {
  subscribe: (req: SubscribeRequest) => void;
  unsubscribe: () => void;
}