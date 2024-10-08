use dino_server::{
    start_server, ProjectConfig, SwappableAppRouter, SwappableWorkerPool, TenentRouter,
    TenentWorkerPool,
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt::Layer, layer::SubscriberExt, util::SubscriberInitExt, Layer as _};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let layer = Layer::new().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let config = include_str!("../fixtures/config.yml");
    let config: ProjectConfig = serde_yaml::from_str(config)?;

    let code = r#"
    (function(){
        async function hello(req){
            return {
                status:200,
                headers:{
                    "content-type":"application/json"
                },
                body: JSON.stringify(req),
            };
        }
        return{hello:hello};
    })();
    "#;

    let routers = vec![TenentRouter::new(
        "localhost",
        SwappableAppRouter::try_new(code, config.routes)?,
    )];

    let pools = vec![TenentWorkerPool::new(
        "localhost",
        SwappableWorkerPool::try_new(code, 10)?,
    )];

    start_server(8888, routers, pools).await?;
    Ok(())
}
