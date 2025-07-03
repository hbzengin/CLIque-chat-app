mod protocol;
mod server;

use dotenv::dotenv;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    println!("Hello, world!");

    let port: i32 = env::var("PORT")?.parse()?;
    let server = server::ChatServer::new(port);
    server.run().await?;
    Ok(())
}
