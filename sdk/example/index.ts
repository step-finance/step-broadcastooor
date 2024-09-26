import { exit } from "process";
import { StepDataDoootBroadcastooor } from "../dist/index.mjs"

//creating the connection only requires the url of the server without the namespace 
//optionally, this can also take an auth token, which is a jwt retrieved from the api login endpoint
const b = new StepDataDoootBroadcastooor("http://localhost:3000");

//theres several state events that can be listened to
b.onConnect(() => {
  console.log("connected");
});
b.onDisconnect(() => {
  console.log("disconnected");
});
b.onSubscribed((message) => {
  console.log(`subscribed: ${message}`);
});
b.onUnsubscribed((message) => {
  console.log(`unsubscribed: ${message}`);
});

//theres also an error event that can be listened to
b.onServerError((error) => {
  console.error(`server error: ${error}`);
});

//the receivedDooot event is where the data is received
b.onReceivedDooot((message) => {
  //the message is a DoootMessage object, which has a dooot property that is the dooot object
  //you can pull out the type using the in operator
  if (message.dooot.t == "SlotStats") {
    console.log(`received slot ${message.dooot.slot}`);
  }
});

//subscribing to a topic is as simple as calling the subscribe method with the topic name
console.log("subscribing to SlotStats");
b.subscribe("SlotStats");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

//you can also subscribe to a topic with a filter. note that this will result in dupe
//messages in this case since the above topic gets all slots and this one gets only even slots
console.log("subscribing to even numbered SlotStats");
b.subscribe("SlotStats", "even-slots", "slot % 2 == 0");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

//you can unsubscribe from a topic by calling the unsubscribe method with the topic name
//this doesn't unsub the filter based subscription, just the generic one
console.log("unsubscribing from SlotStats");
b.unsubscribe("SlotStats");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

//you can also unsubscribe from a filter based subscription by calling the unsubscribe method
//with the topic name and the filter id - in this case, we demonstrate the error that gets sent
//when you try to unsubscribe from a filter id that doesn't exist
console.log("unsubscribing from invalid filter id");
b.unsubscribe("SlotStats", "odd-slots");

//this will unsubscribe from the even slots filter
console.log("unsubscribing from even numbered SlotStats");
b.unsubscribe("SlotStats", "even-slots");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

//subscribe to odd slots
console.log("subscribing to odd numbered SlotStats");
b.subscribe("SlotStats", "odd-slots", "slot % 2 == 1");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

//unsub from odd
console.log("unsubscribing from odd numbered SlotStats");
b.unsubscribe("SlotStats", "odd-slots");

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

//closing will remove all listeners and close the socket
console.log("closing connection");
b.close();

console.log("waiting 5 seconds");
await new Promise((resolve) => setTimeout(resolve, 5000));

console.log("done");
exit(0);