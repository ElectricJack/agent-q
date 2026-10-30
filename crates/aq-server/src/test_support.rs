//! Per-test PostgreSQL databases. Needs `DATABASE_URL` (see `scripts/test-db`).

use crate::store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgConnection, PgPool};
use std::str::FromStr;
use uuid::Uuid;

pub struct TestDb {
    admin_url: String,
    name: String,
    options: PgConnectOptions,
}

impl TestDb {
    pub async fn create() -> TestDb {
        let admin_url =
            std::env::var("DATABASE_URL").expect("set DATABASE_URL via scripts/test-db");
        let name = format!("aq_test_{}", Uuid::new_v4().simple());
        let mut conn = PgConnection::connect(&admin_url)
            .await
            .expect("connect to DATABASE_URL");
        sqlx::query(&format!("CREATE DATABASE {name}"))
            .execute(&mut conn)
            .await
            .expect("create test database");
        let _ = conn.close().await;
        let options = PgConnectOptions::from_str(&admin_url)
            .expect("DATABASE_URL is a valid PostgreSQL URL")
            .database(&name);
        TestDb {
            admin_url,
            name,
            options,
        }
    }

    /// Connect options (host, credentials, query parameters of `DATABASE_URL`)
    /// pointing at this test's database.
    pub fn options(&self) -> PgConnectOptions {
        self.options.clone()
    }

    /// Connects (and migrates) a fresh `Store` to this database.
    pub async fn store(&self) -> Store {
        Store::connect_with(self.options())
            .await
            .expect("connect store")
    }

    /// A raw pool for tests that poke at tables directly.
    pub async fn pool(&self) -> PgPool {
        PgPoolOptions::new()
            .max_connections(2)
            .connect_with(self.options())
            .await
            .expect("connect pool")
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let admin_url = self.admin_url.clone();
        let name = self.name.clone();
        let _ = std::thread::spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(async {
                if let Ok(mut conn) = PgConnection::connect(&admin_url).await {
                    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {name} WITH (FORCE)"))
                        .execute(&mut conn)
                        .await;
                }
            });
        })
        .join();
    }
}
