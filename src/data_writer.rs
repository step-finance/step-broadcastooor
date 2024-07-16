use tokio::{sync::mpsc, time::sleep};
use tokio_postgres::{types::ToSql, NoTls};

pub struct ApiLog {
    pub auth_key: String,
    pub auth_type: i16,
    pub endpoint: String,
    pub query_params: serde_json::Value,
    pub start_time: i64,
    pub end_time: i64,
    pub ip_address: String,
    pub status_code: i32,
}

impl ApiLog {
    #[inline]
    pub fn to_db_params(&self) -> [&(dyn ToSql + Sync); 8] {
        [
            &self.auth_key,
            &self.auth_type,
            &self.endpoint,
            &self.query_params,
            &self.start_time,
            &self.end_time,
            &self.ip_address,
            &self.status_code,
        ]
    }
}

pub async fn create_database_writer_task(
    mut rx: mpsc::UnboundedReceiver<ApiLog>,
    con_string: Option<String>,
) {
    // If there is no connection string, just loop forever
    if con_string.is_none() {
        while rx.recv().await.is_some() {}
        return;
    }

    //this is database reconnect loop
    loop {
        log::debug!("connecting to database");
        let con_string = con_string.clone().unwrap();
        let (client, connection) = match tokio_postgres::connect(&con_string, NoTls).await {
            Ok((client, connection)) => (client, connection),
            Err(e) => {
                log::error!("Error connecting to database: {}", e);
                sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        // The connection object performs the actual communication with the database,
        // so spawn it off to run on its own.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        let query = "call auth.fn_insert_time_api_log($1, $2, $3, $4, $5, $6, $7, $8)";
        let stmt = client.prepare(query).await.unwrap();
        //now loop on the receiver and write to the database
        while let Some(log) = rx.recv().await {
            let params = &log.to_db_params();
            if let Err(e) = client.execute(&stmt, params).await {
                log::error!("Error writing to database: {}", e);
                //break out to the retry logic
                break;
            }
        }
    }
}
