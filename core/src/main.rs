// main.rs — thin entrypoint. All server logic lives in lib.rs::run().
//
// The binary target and the library target are separate compilation units in
// the same package. Here we reference the library via its crate name `Qubit`
// (matching the `name` field in Cargo.toml).

use tokio::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let config = Qubit::config::init_config();
    log::info!("Config loaded: {:?}", config);

    // Ctrl-C is handled here rather than inside `run()` so that load tests
    // can call `run()` directly and control lifetime by dropping the task.
    tokio::select! {
        result = Qubit::run(config) => result,
        _ = signal::ctrl_c() => {
            log::info!("shutdown signal received");
            Ok(())
        }
    }
}
