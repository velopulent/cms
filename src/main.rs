mod database;
mod middleware {
    pub mod auth;
}
mod models {
    pub mod content;
    pub mod schema;
    pub mod site;
    pub mod user;
}
mod handlers {
    pub mod auth_handler;
    pub mod content_handler;
    pub mod schema_handler;
    pub mod site_handler;
    pub mod ui_handler;
}
mod router;

use std::net::SocketAddr;
use bcrypt::{hash, DEFAULT_COST};
use uuid::Uuid;
use router::create_router;
use database::init_db;

#[tokio::main]
async fn main() {
    let pool = init_db().await;

    seed_admin(&pool).await;

    let app = create_router(pool);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Server running on {}", addr);

    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app,
    )
    .await
    .unwrap();
}

async fn seed_admin(pool: &sqlx::SqlitePool) {
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE username = 'admin'")
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

    if exists.is_none() {
        let id = Uuid::now_v7().to_string();
        let password_hash = hash("admin", DEFAULT_COST).expect("Failed to hash password");
        sqlx::query("INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind("admin")
            .bind("admin@cms.local")
            .bind(&password_hash)
            .execute(pool)
            .await
            .expect("Failed to seed admin user");

        println!("Seeded default admin user (admin/admin)");
    }
}
