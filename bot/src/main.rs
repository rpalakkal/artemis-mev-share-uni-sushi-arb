use std::sync::Arc;

use artemis_core::{
    collectors::mevshare_collector::MevShareCollector,
    engine::Engine,
    executors::mev_share_executor::MevshareExecutor,
    types::{CollectorMap, ExecutorMap},
};
use ethers::{
    prelude::MiddlewareBuilder,
    providers::{Provider, Ws},
    signers::{LocalWallet, Signer},
    types::{Address, Chain},
};

use clap::Parser;

use tracing::{info, Level};
use tracing_subscriber::{filter, prelude::*};
use uni_sushi::{Action, Event, MevShareUniSushiArb};

mod uni_sushi;

use eyre::Result;

/// CLI Options.
#[derive(Parser, Debug)]
pub struct Args {
    /// Ethereum node WS endpoint.
    #[arg(long)]
    pub wss: String,
    /// Private key for sending txs.
    #[arg(long)]
    pub private_key: String,
    /// MEV share signer
    #[arg(long)]
    pub flashbots_signer: String,
    /// Address of the arb contract.
    #[arg(long)]
    pub arb_contract_address: Address,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = filter::Targets::new()
        .with_target("bot", Level::INFO)
        .with_target("artemis_core", Level::INFO);
    tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).with(filter).init();
    // Set up engine.
    let mut engine: Engine<Event, Action> = Engine::default();

    let args = Args::parse();

    // Set up collector.
    let mevshare_collector =
        Box::new(MevShareCollector::new(String::from("https://mev-share.flashbots.net")));
    let mevshare_collector = CollectorMap::new(mevshare_collector, Event::MEVShareEvent);
    engine.add_collector(Box::new(mevshare_collector));

    //  Set up providers and signers.
    let ws = Ws::connect(args.wss).await?;
    let provider = Provider::new(ws);

    let wallet: LocalWallet = args.private_key.parse().unwrap();
    let address = wallet.address();
    let contract_address = args.arb_contract_address;

    let provider = Arc::new(provider.nonce_manager(address).with_signer(wallet.clone()));
    let fb_signer: LocalWallet = args.flashbots_signer.parse().unwrap();

    let provider = Arc::new(provider.nonce_manager(address).with_signer(wallet.clone()));
    let strategy = MevShareUniSushiArb::new(Arc::new(provider.clone()), wallet, contract_address);
    engine.add_strategy(Box::new(strategy));

    // Set up executor.

    let mev_share_executor = Box::new(MevshareExecutor::new(fb_signer, Chain::Mainnet));
    let mev_share_executor = ExecutorMap::new(mev_share_executor, |action| match action {
        Action::SubmitBundles(bundles) => Some(bundles),
    });
    engine.add_executor(Box::new(mev_share_executor));

    // Start engine.
    if let Ok(mut set) = engine.run().await {
        while let Some(res) = set.join_next().await {
            info!("res: {:?}", res);
        }
    }

    Ok(())
}
