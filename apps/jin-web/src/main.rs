mod web;

use clap::Parser;
use std::net::SocketAddr;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8788")]
    addr: SocketAddr,
    #[arg(long, env = "JIN_API_BASE", default_value = "http://127.0.0.1:8787")]
    api_base: String,
    #[arg(long, env = "JIN_API_TOKEN")]
    api_token: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(error) = web::serve(args.addr, args.api_base, args.api_token).await {
        eprintln!("jin-web failed: {error}");
        std::process::exit(1);
    }
}
