use dotenv::dotenv;

use sqlx::postgres::PgPoolOptions;
use std::env;

use juggernaut_broker::{db::Database, http};

// pub static MIGRATOR: Migrator = sqlx::migrate!(); // defaults to "./migrations"
// use sqlx::mysql::MySqlPoolOptions;
// etc.

#[tokio::main]
// or #[tokio::main]
// or #[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    dotenv().ok();
    let database_url =
        env::var("DATABASE_URL").map_err(|_err| anyhow::anyhow!("Please specify database url"))?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    let database = Database::new(pool);
    println!("Starting server");
    http::serve(("127.0.0.1", 8080), database).await?;
    Ok(())
}
