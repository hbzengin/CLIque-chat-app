// main.rs

mod client;
use std::{env, error::Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "127.0.0.1".into());
    let port = args.next().unwrap_or_else(|| "8080".into());
    let client = client::ChatClient::new(host, port).await?;
    client.run().await
}
