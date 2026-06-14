use rmcp::ServiceExt;
use rmcp::transport::stdio;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let server = codex_nav_mcp_server::NavMcpServer::new();
    let running: rmcp::service::RunningService<rmcp::service::RoleServer, _> =
        server.serve(stdio()).await?;
    running.waiting().await?;
    Ok(())
}
