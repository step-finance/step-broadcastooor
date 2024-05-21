See Rust docs for details on schemas and their topics:
https://step-finance.github.io/step-broadcastooor

samples

`subscribe`
`{ "topic": "SlotStats", "filter": { "id": "1", "expression": "slot % 2 == 0" } }`

`unsubscribe`
`{ "topic": "SlotStats", "filter_id": "1" }`