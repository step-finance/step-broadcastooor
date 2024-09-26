import { DoootMessage } from "./types/DoootMessage";
import { SubscribeRequest } from "./types/SubscribeRequest";
import { UnsubscribeRequest } from "./types/UnsubscribeRequest";

export interface DataDoootListenEvents {
  subscribed: (topic: string) => void;
  unsubscribed: (topic: string) => void;
  serverError: (error: string) => void;
  receivedDooot: (message: DoootMessage) => void;
}

export interface DataDoootEmitEvents {
  subscribe: (req: SubscribeRequest) => void;
  unsubscribe: (req: UnsubscribeRequest) => void;
}