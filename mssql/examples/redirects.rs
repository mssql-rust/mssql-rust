//! Handling a server-requested redirect.
//!
//! With certain Azure firewall settings, a login might return
//! `Error::Routing { host, port }`. This means the client must open a new
//! `TcpStream` to the given address and connect again — there should never
//! be more than one redirect, so this short-circuits after the first.
use mssql::{error::Error, AuthMethod, Client, Config};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut config = Config::new();

    config.host("0.0.0.0");
    config.port(1433);
    config.authentication(AuthMethod::sql_server("SA", "<Mys3cureP4ssW0rD>"));

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let client = match Client::connect(config, tcp.compat_write()).await {
        // Connection successful.
        Ok(client) => client,
        // The server wants us to redirect to a different address.
        Err(Error::Routing { host, port }) => {
            let mut config = Config::new();

            config.host(&host);
            config.port(port);
            config.authentication(AuthMethod::sql_server("SA", "<Mys3cureP4ssW0rD>"));

            let tcp = TcpStream::connect(config.get_addr()).await?;
            tcp.set_nodelay(true)?;

            Client::connect(config, tcp.compat_write()).await?
        }
        Err(e) => Err(e)?,
    };
    let _ = client;

    Ok(())
}
