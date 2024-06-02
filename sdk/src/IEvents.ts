import { SchemaMessage } from "./types/SchemaMessage";
import { SubscribeRequest } from "./types/SubscribeRequest";
import { UnsubscribeRequest } from "./types/UnsubscribeRequest";

export interface DataSchemaListenEvents {
  subscribed: (topic: string) => void;
  unsubscribed: (topic: string) => void;
  serverError: (error: string) => void;
  receivedSchema: (message: SchemaMessage) => void;
}

export interface DataSchemaEmitEvents {
  subscribe: (req: SubscribeRequest) => void;
  unsubscribe: (req: UnsubscribeRequest) => void;
}