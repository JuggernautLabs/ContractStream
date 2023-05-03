mod db_utils;
mod http;
mod models;

use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

pub static MIGRATOR: Migrator = sqlx::migrate!(); // defaults to "./migrations"
                                                  // use sqlx::mysql::MySqlPoolOptions;
                                                  // etc.

#[tokio::main]
// or #[tokio::main]
// or #[actix_web::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create a connection pool
    //  for MySQL, use MySqlPoolOptions::new()
    //  for SQLite, use SqlitePoolOptions::new()
    //  etc.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect("postgres://shmendez@localhost/gptftw")
        .await?;

    MIGRATOR.run(&pool).await?;
    pool.close().await;
    // let user = UserBuilder::default()
    //     .username("mendez".into())
    //     .password("password".into())
    //     .build_from_db(pool)
    //     .await?;

    // let user = User::add_user("shabram".into(), "123123".into(), pool).await?;
    // println!("{:?}", user);
    //
    http::serve().await?;
    Ok(())
}
