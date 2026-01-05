use log::info;

mod config;
mod loader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config = config::init_config();

    let loader = loader::EbpfLoader::new(config.clone(), config.perf_array_name.clone());
    
    loader.start().await
}
