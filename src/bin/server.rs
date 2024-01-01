use clap::{Parser};
use pulcinella::server::{bind_socket, run_controlplane, run_mock, Mode, SequentialState};
use std::net::SocketAddr;
use tokio::join;
use tracing::{info, level_filters::LevelFilter, Level};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Opts {
    #[clap(short, long, default_value = "false", env = "PROXY")]
    proxy_mode: bool,
    #[clap(short, long, default_value = "0", env = "CONTROL_PORT")]
    control_port: u16,
    #[clap(short, long, default_value = "0", env = "MOCK_PORT")]
    mock_port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let opts = Opts::parse();

    let subscriber = FmtSubscriber::builder()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_max_level(Level::INFO)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let control_addr = SocketAddr::from(([127, 0, 0, 1], opts.control_port));
    let mock_addr = SocketAddr::from(([127, 0, 0, 1], opts.mock_port));

    let mock = bind_socket(mock_addr).await?;
    let control = bind_socket(control_addr).await?;
    let state = SequentialState::new(mock.port);

    info!("Control Port on http://127.0.0.1:{}/", control.port);
    info!("{} on http://127.0.0.1:{}/", if opts.proxy_mode { "Proxy" } else { "Mock" }, mock.port);

    let control = run_controlplane(control.listener, state.clone());
    let mock = run_mock(mock.listener, state, if opts.proxy_mode { Mode::Proxy } else { Mode::Mock });
    let (cp_result, mock_result) = join!(control, mock);

    cp_result.and(mock_result)
}
