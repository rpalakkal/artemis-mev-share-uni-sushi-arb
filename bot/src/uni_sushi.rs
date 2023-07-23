use std::collections::HashMap;
use std::ops::Add;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use anyhow::Result;
use artemis_core::types::Strategy;

use ethers::signers::Signer;
use matchmaker::types::{BundleRequest, BundleTx};

use ethers::providers::Middleware;
use ethers::types::{Address, H256};
use ethers::types::{H160, U256};
use tracing::info;

use artemis_core::executors::mev_share_executor::Bundles;

use mev_share::sse;

use bindings::blind_arb::BlindArb;

/// Core Event enum for the current strategy.
#[derive(Debug, Clone)]
pub enum Event {
    MEVShareEvent(sse::Event),
}

/// Core Action enum for the current strategy.
#[derive(Debug, Clone)]
pub enum Action {
    SubmitBundles(Bundles),
}

#[derive(Debug, serde::Deserialize)]
pub struct PoolRecord {
    pub token_address: H160,
    pub uni_pool_address: H160,
    pub sushi_pool_address: H160,
}

#[derive(Debug, Clone)]
pub struct MevShareUniSushiArb<M, S> {
    /// Ethers client.
    client: Arc<M>,
    /// Maps uni uni pool address to sushi pool information.
    uni_pool_map: HashMap<H160, H160>,
    /// Maps uni sushi pool address to  pooluni information.
    sushi_pool_map: HashMap<H160, H160>,
    /// Signer for transactions.
    tx_signer: S,
    /// Arb contract.
    arb_contract: BlindArb<M>,
}

impl<M: Middleware + 'static, S: Signer> MevShareUniSushiArb<M, S> {
    /// Create a new instance of the strategy.
    pub fn new(client: Arc<M>, signer: S, arb_contract_address: Address) -> Self {
        Self {
            client: client.clone(),
            uni_pool_map: HashMap::new(),
            sushi_pool_map: HashMap::new(),
            tx_signer: signer,
            arb_contract: BlindArb::new(arb_contract_address, client),
        }
    }
}

#[async_trait]
impl<M: Middleware + 'static, S: Signer + 'static> Strategy<Event, Action>
    for MevShareUniSushiArb<M, S>
{
    /// Initialize the strategy. This is called once at startup, and loads
    /// pool information into memory.
    async fn sync_state(&mut self) -> Result<()> {
        // Read pool information from csv file.
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/uni_sushi_weth_pools.csv");
        let mut reader = csv::Reader::from_path(path)?;

        for record in reader.deserialize() {
            // Parse records into PoolRecord struct.
            let record: PoolRecord = record?;
            self.uni_pool_map.insert(record.uni_pool_address, record.sushi_pool_address);
            self.sushi_pool_map.insert(record.sushi_pool_address, record.uni_pool_address);
        }

        Ok(())
    }

    // Process incoming events, seeing if we can arb new orders.
    async fn process_event(&mut self, event: Event) -> Option<Action> {
        match event {
            Event::MEVShareEvent(event) => {
                info!("Received mev share event: {:?}", event);
                // skip if event has no logs
                if event.logs.is_empty() {
                    return None;
                }
                let address = event.logs[0].address;

                // skip if address is not a v3 pool
                if !self.sushi_pool_map.contains_key(&address)
                    && !self.uni_pool_map.contains_key(&address)
                {
                    return None;
                }

                info!("Found a pool match at address {:?}, submitting bundles", address);
                let bundles = self.generate_bundles(address, event.hash).await;
                return Some(Action::SubmitBundles(bundles));
            }
        }
    }
}

impl<M: Middleware + 'static, S: Signer + 'static> MevShareUniSushiArb<M, S> {
    /// Generate a series of bundles of varying sizes to submit to the matchmaker.
    pub async fn generate_bundles(&self, address: H160, tx_hash: H256) -> Vec<BundleRequest> {
        let mut bundles = Vec::new();
        // let v2_info = self.pool_map.get(&v3_address).unwrap();

        let address_2 = if self.sushi_pool_map.contains_key(&address) {
            self.sushi_pool_map.get(&address).unwrap()
        } else {
            self.uni_pool_map.get(&address).unwrap()
        };

        // The sizes of the backruns we want to submit.
        // TODO: Run some analysis to figure out likely sizes.
        let sizes = vec![
            U256::from(100000_u128),
            U256::from(1000000_u128),
            U256::from(10000000_u128),
            U256::from(100000000_u128),
            U256::from(1000000000_u128),
            U256::from(10000000000_u128),
            U256::from(100000000000_u128),
            U256::from(1000000000000_u128),
            U256::from(10000000000000_u128),
            U256::from(100000000000000_u128),
            U256::from(1000000000000000_u128),
            U256::from(10000000000000000_u128),
            U256::from(100000000000000000_u128),
            U256::from(1000000000000000000_u128),
        ];

        // Set parameters for the backruns.
        let payment_percentage = U256::from(0);
        let bid_gas_price = self.client.get_gas_price().await.unwrap();
        let block_num = self.client.get_block_number().await.unwrap();

        for size in sizes {
            let arb_tx = {
                // Construct arb tx based on whether the v2 pool has weth as token0.
                let mut inner = self
                    .arb_contract
                    .execute_arb(address, *address_2, size, payment_percentage)
                    .tx;

                // Set gas parameters (this is a bit hacky)
                inner.set_gas(400000);
                inner.set_gas_price(bid_gas_price);
                let fill = self.client.fill_transaction(&mut inner, None).await;

                match fill {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Error filling tx: {}", e);
                        continue;
                    }
                }

                inner
            };
            info!("generated arb tx: {:?}", arb_tx);

            // Sign tx and construct bundle
            let signature = self.tx_signer.sign_transaction(&arb_tx).await.unwrap();
            let bytes = arb_tx.rlp_signed(&signature);
            let txs = vec![
                BundleTx::TxHash { hash: tx_hash },
                BundleTx::Tx { tx: bytes, can_revert: false },
            ];
            // bundle should be valid for next block
            let bundle = BundleRequest::make_simple(block_num.add(1), txs);
            info!("submitting bundle: {:?}", bundle);
            bundles.push(bundle);
        }
        //vec![]
        bundles
    }
}
