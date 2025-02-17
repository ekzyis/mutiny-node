use crate::error::MutinyError;
use crate::keymanager::PhantomKeysManager;
use crate::ldkstorage::PhantomChannelManager;
use crate::logging::MutinyLogger;
use crate::node::LiquidityManager;
use crate::storage::MutinyStorage;
use async_trait::async_trait;
use bitcoin::secp256k1::PublicKey;
use bitcoin::Network;
use lightning::ln::PaymentHash;
use lightning_invoice::Bolt11Invoice;
use lsps::{LspsClient, LspsConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{atomic::AtomicBool, Arc};
use voltage::LspClient;

pub mod lsps;
pub mod voltage;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LspConfig {
    VoltageFlow(String),
    Lsps(LspsConfig),
}

impl LspConfig {
    pub fn new_voltage_flow(url: String) -> Self {
        Self::VoltageFlow(url)
    }

    pub fn new_lsps(connection_string: String, token: Option<String>) -> Self {
        Self::Lsps(LspsConfig {
            connection_string,
            token,
        })
    }

    pub fn accept_underpaying_htlcs(&self) -> bool {
        match self {
            LspConfig::VoltageFlow(_) => false,
            LspConfig::Lsps(_) => true,
        }
    }
}

pub fn deserialize_lsp_config<'de, D>(deserializer: D) -> Result<Option<LspConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v: Option<Value> = Option::deserialize(deserializer)?;
    match v {
        Some(Value::String(s)) => Ok(Some(LspConfig::VoltageFlow(s))),
        Some(Value::Object(_)) => LspConfig::deserialize(v.unwrap())
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("invalid lsp config: {e}"))),
        Some(Value::Null) => Ok(None),
        Some(x) => Err(serde::de::Error::custom(format!(
            "invalid lsp config: {x:?}"
        ))),
        None => Ok(None),
    }
}

#[derive(Serialize, Deserialize)]
pub struct InvoiceRequest {
    // Used only for VoltageFlow
    pub bolt11: Option<String>,
    // Used only for VoltageFlow to map to previously fetched fee
    pub fee_id: Option<String>,
    // Used only for LSPS to track channel creation
    pub user_channel_id: Option<u128>,
}

#[derive(Serialize, Deserialize)]
pub struct FeeRequest {
    pub pubkey: String,
    pub amount_msat: u64,
    // Used only for LSPS to track channel creation
    pub user_channel_id: Option<u128>,
}

#[derive(Serialize, Deserialize)]
pub struct FeeResponse {
    // Used only for VoltageFlow to be used in subsequent InvoiceRequest
    pub id: Option<String>,
    pub fee_amount_msat: u64,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub(crate) trait Lsp {
    async fn get_lsp_fee_msat(&self, fee_request: FeeRequest) -> Result<FeeResponse, MutinyError>;
    async fn get_lsp_invoice(
        &self,
        invoice_request: InvoiceRequest,
    ) -> Result<Bolt11Invoice, MutinyError>;
    fn get_lsp_pubkey(&self) -> PublicKey;
    fn get_lsp_connection_string(&self) -> String;
    fn get_expected_skimmed_fee_msat(&self, payment_hash: PaymentHash, payment_size: u64) -> u64;
    fn get_config(&self) -> LspConfig;
}

#[derive(Clone)]
pub enum AnyLsp<S: MutinyStorage> {
    VoltageFlow(LspClient),
    Lsps(LspsClient<S>),
}

impl<S: MutinyStorage> AnyLsp<S> {
    pub async fn new_voltage_flow(url: &str) -> Result<Self, MutinyError> {
        Ok(Self::VoltageFlow(LspClient::new(url).await?))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_lsps(
        connection_string: String,
        token: Option<String>,
        liquidity_manager: Arc<LiquidityManager<S>>,
        channel_manager: Arc<PhantomChannelManager<S>>,
        keys_manager: Arc<PhantomKeysManager<S>>,
        network: Network,
        logger: Arc<MutinyLogger>,
        stop: Arc<AtomicBool>,
    ) -> Result<Self, MutinyError> {
        let lsps_client = LspsClient::new(
            connection_string,
            token,
            liquidity_manager,
            channel_manager,
            keys_manager,
            network,
            logger,
            stop,
        )?;
        Ok(Self::Lsps(lsps_client))
    }

    pub fn accept_underpaying_htlcs(&self) -> bool {
        match self {
            AnyLsp::VoltageFlow(_) => false,
            AnyLsp::Lsps(_) => true,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: MutinyStorage> Lsp for AnyLsp<S> {
    async fn get_lsp_fee_msat(&self, fee_request: FeeRequest) -> Result<FeeResponse, MutinyError> {
        match self {
            AnyLsp::VoltageFlow(client) => client.get_lsp_fee_msat(fee_request).await,
            AnyLsp::Lsps(client) => client.get_lsp_fee_msat(fee_request).await,
        }
    }

    async fn get_lsp_invoice(
        &self,
        invoice_request: InvoiceRequest,
    ) -> Result<Bolt11Invoice, MutinyError> {
        match self {
            AnyLsp::VoltageFlow(client) => client.get_lsp_invoice(invoice_request).await,
            AnyLsp::Lsps(client) => client.get_lsp_invoice(invoice_request).await,
        }
    }

    fn get_lsp_pubkey(&self) -> PublicKey {
        match self {
            AnyLsp::VoltageFlow(client) => client.get_lsp_pubkey(),
            AnyLsp::Lsps(client) => client.get_lsp_pubkey(),
        }
    }

    fn get_lsp_connection_string(&self) -> String {
        match self {
            AnyLsp::VoltageFlow(client) => client.get_lsp_connection_string(),
            AnyLsp::Lsps(client) => client.get_lsp_connection_string(),
        }
    }

    fn get_config(&self) -> LspConfig {
        match self {
            AnyLsp::VoltageFlow(client) => client.get_config(),
            AnyLsp::Lsps(client) => client.get_config(),
        }
    }

    fn get_expected_skimmed_fee_msat(&self, payment_hash: PaymentHash, payment_size: u64) -> u64 {
        match self {
            AnyLsp::VoltageFlow(client) => {
                client.get_expected_skimmed_fee_msat(payment_hash, payment_size)
            }
            AnyLsp::Lsps(client) => {
                client.get_expected_skimmed_fee_msat(payment_hash, payment_size)
            }
        }
    }
}
