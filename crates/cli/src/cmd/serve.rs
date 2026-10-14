use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use anyhow::{Context as _, Result};
use overrules::Network;

#[derive(clap::Args)]
pub struct Args {
    /// Address to listen on.
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    host: IpAddr,
    /// Port to listen on.
    #[arg(long, default_value_t = 8787)]
    port: u16,
}

pub fn run(net: Network, a: &Args) -> Result<()> {
    let addr = SocketAddr::new(a.host, a.port);
    let runtime = tokio::runtime::Runtime::new()?;
    eprintln!("overrules explorer → http://{addr}  (Ctrl-C to stop)");
    runtime
        .block_on(overrules_explorer::server::serve(net, addr))
        .with_context(|| format!("serving on {addr}"))
}
