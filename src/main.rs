mod protocol;
mod server;

use dotenv::dotenv;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    let port: i32 = env::var("PORT")?.parse()?;
    println!("Starting server on port {port}!");
    let server = server::ChatServer::new(port);
    server.run().await?;
    Ok(())
}
