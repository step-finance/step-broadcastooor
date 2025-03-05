use tokio::{sync::mpsc, time::sleep};
use tokio_postgres::{types::ToSql, NoTls};

use crate::handlers::connect::ConnectedUserInfo;

#[derive(Debug)]
pub struct ApiLog {
    pub auth_key: Option<String>,
    pub auth_type: Option<i16>,
    pub endpoint: String,
    pub query_params: Option<serde_json::Value>,
    pub start_time: i64,
    pub end_time: i64,
    pub ip_address: Option<String>,
    pub status_code: Option<i32>,
    pub referer: Option<String>,
    pub cost: Option<i16>,
}

impl ApiLog {
    #[inline]
    pub fn to_db_params(&self) -> [&(dyn ToSql + Sync); 10] {
        [
            &self.auth_key,
            &self.auth_type,
            &self.endpoint,
            &self.query_params,
            &self.start_time,
            &self.end_time,
            &self.ip_address,
            &self.status_code,
            &self.referer,
            &self.cost,
        ]
    }
    #[inline]
    pub fn from_user(user: &ConnectedUserInfo) -> Self {
        let claims = user.claims.as_ref();
        let now = chrono::Utc::now().timestamp();
        Self {
            auth_key: claims
                .and_then(|c| c.api_key.as_ref().or(c.public_key.as_ref()))
                .cloned(),
            auth_type: claims.and_then(|c| {
                if c.api_key.is_some() {
                    Some(1)
                } else if c.public_key.is_some() {
                    Some(2)
                } else {
                    None
                }
            }),
            endpoint: "/dooots".to_string(),
            query_params: None,
            start_time: now,
            end_time: now,
            ip_address: user.ip_address.clone(),
            status_code: None,
            referer: None,
            cost: None,
        }
    }
}

pub async fn create_database_writer_task(
    mut rx: mpsc::UnboundedReceiver<ApiLog>,
    con_string: Option<String>,
) {
    // If there is no connection string, just loop forever
    if con_string.is_none() {
        while let Some(a) = rx.recv().await {
            log::debug!("ApiLog: {:?}", a);
        }
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

        let query =
            "call auth.fn_insert_time_api_log_with_cost($1, $2, $3, $4, $5, $6, $7, $8, $9, $10);";
        let stmt = client.prepare(query).await.unwrap();
        //now loop on the receiver and write to the database
        while let Some(log) = rx.recv().await {
            let params = &log.to_db_params();
            if let Err(e) = client.execute(&stmt, params).await {
                log::error!("Error writing to database: {}", e);
                //break out to the reconnect logic after a pause to prevent fast loops on err
                sleep(std::time::Duration::from_secs(5)).await;
                break;
            }
        }
    }
}
