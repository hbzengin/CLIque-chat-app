use protocol;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port: i32 = 8080;
    println!("Starting server on port {port}!");
    let server = server::ChatServer::new(port);
    server.run().await?;
    Ok(())
}
