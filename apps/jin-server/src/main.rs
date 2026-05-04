mod api;

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8787")]
    addr: SocketAddr,
    #[arg(long, default_value = ".jin/state/state.json")]
    state: PathBuf,
    #[arg(long, env = "JIN_API_TOKEN")]
    api_token: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(error) = api::serve(args.addr, args.state, args.api_token).await {
        eprintln!("jin-server failed: {error}");
        std::process::exit(1);
    }
}
