import { exit } from "process";
import { StepDataSchemaBroadcastooor } from "../dist/index.mjs"

const b = new StepDataSchemaBroadcastooor("http://localhost:3000/data_schema");
b.onSubscribed((message) => {
  console.log(`subscribed: ${message}`);
});
b.onUnsubscribed((message) => {
  console.log(`unsubscribed: ${message}`);
});
b.onServerError((error) => {
  console.error(`server error: ${error}`);
});
b.onReceivedSchema((message) => {
  if ("SlotStats" in message.schema) {
    console.log(`received slot ${message.schema.SlotStats.slot}`);
  }
});

console.log("subscribing to SlotStats");
b.subscribe("SlotStats");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

console.log("subscribing to even numbered SlotStats");
b.subscribe("SlotStats", { id: "1", "expression": "slot % 2 = 0" });

console.log("unsubscribing from SlotStats");
b.unsubscribe("SlotStats");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

console.log("closing connection");
b.close();

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

console.log("done");
exit(0);