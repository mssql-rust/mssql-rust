use mssql::{Client, Config, ProcParam};
use once_cell::sync::Lazy;
use std::env;
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::log::info;

static CONN_STR: Lazy<String> = Lazy::new(|| {
    env::var("MSSQL_TEST_CONNECTION_STRING").unwrap_or_else(|_| {
        "server=tcp:localhost,1433;IntegratedSecurity=true;TrustServerCertificate=true".to_owned()
    })
});

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = Config::from_ado_string(&CONN_STR)?;

    let tcp = TcpStream::connect(config.get_addr()).await?;
    tcp.set_nodelay(true)?;

    let mut client = Client::connect(config, tcp.compat_write()).await?;

    client
        .simple_query(
            "CREATE OR ALTER PROCEDURE ##double_it (@n INT, @doubled INT OUTPUT) AS
             BEGIN SET @doubled = @n * 2; RETURN 1; END",
        )
        .await?;
    info!("procedure created");

    let stream = client
        .call_procedure(
            "##double_it",
            &[
                ProcParam::input("@n", &21i32),
                // The 0 is just a placeholder establishing the type - the
                // procedure overwrites it, we never see this value again.
                // An OUTPUT parameter's placeholder must be a concrete,
                // non-NULL value (not e.g. &None::<i32>) - SQL Server
                // rejects an untyped NULL bound as OUTPUT.
                ProcParam::output("@doubled", &0i32),
            ],
        )
        .await?;

    // This procedure has no SELECT, so there are no result-set rows to
    // read before this - if it did, read those first via the `Stream`
    // API, same as with `Client::query`.
    let outputs = stream.into_output_params().await?;

    info!("doubled = {:?}", outputs.get::<i32>("@doubled"));
    info!("return status = {:?}", outputs.return_status());

    Ok(())
}
