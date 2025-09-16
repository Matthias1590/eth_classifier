use pyo3::prelude::*;
use anyhow::{Result, anyhow};
use std::{fmt::Display, sync::Arc};

mod utils;

pub enum WalletClass {
    Customer,
    MevBot,
    Exchange { hot: bool },
    Contract,
}

impl Display for WalletClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalletClass::Customer => write!(f, "customer"),
            WalletClass::MevBot => write!(f, "mev bot"),
            WalletClass::Exchange { hot } => {
                write!(f, "{} exchange", if *hot { "hot" } else { "cold" })
            },
            WalletClass::Contract => write!(f, "contract"),
        }
    }
}

pub struct WalletClassPrediction {
    pub class: WalletClass,
    pub confidence: f32,
}

pub struct WalletClassifier {
    client: Arc<etherscan::Client>,
}

impl WalletClassifier {
    pub fn new(client: Arc<etherscan::Client>) -> Self {
        Self { client }
    }

    // FIXME: Use libtorch more, all these cpu loops are a waste of time
    pub async fn get_features(&self, address: &str) -> Result<Option<Vec<f32>>> {
        let address = address.to_lowercase();

        let txs = self.client.get_transactions(&address).await?;
        if txs.len() < 2 {
            return Ok(None);
        }

        // FIXME: This assumes the transactions are sorted by timestamp, only true because the api returns them that way (I think), so either ensure that or sort them somewhere
        let start_ts = txs.last().unwrap().timestamp;
        let end_ts = txs.first().unwrap().timestamp;
        let lifetime_s = end_ts - start_ts;
        let lifetime_days = lifetime_s / (24 * 60 * 60);

        let tx_intervals = txs
            .windows(2)
            .map(|w| {
                let end = w[0].timestamp;
                let start = w[1].timestamp;
                end - start
            })
            .collect::<Vec<_>>();

        let incoming_txs: Vec<_> = txs
            .iter()
            .filter(|tx| tx.to.to_lowercase() == address)
            .collect();
        let outgoing_txs: Vec<_> = txs
            .iter()
            .filter(|tx| tx.from.to_lowercase() == address)
            .collect();

        let from_exchanges = incoming_txs
            .iter()
            .filter(|tx| {
                let from = tx.from.to_lowercase();
                utils::is_exchange_owned(&from)
            })
            .count();
        let to_exchanges = outgoing_txs
            .iter()
            .filter(|tx| {
                let to = tx.to.to_lowercase();
                utils::is_exchange_owned(&to)
            })
            .count();

        let tx_values = txs
            .iter()
            .map(|tx| tx.value as f64)
            .collect::<Vec<_>>();
        let ingoing_volume = incoming_txs
            .iter()
            .map(|tx| tx.value as f64)
            .sum::<f64>();
        let outgoing_volume = outgoing_txs
            .iter()
            .map(|tx| tx.value as f64)
            .sum::<f64>();

        let from_addrs = incoming_txs
            .iter()
            .map(|tx| tx.from.to_lowercase())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let to_addrs = outgoing_txs
            .iter()
            .map(|tx| tx.to.to_lowercase())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let addr_reuse = (from_addrs + to_addrs) / txs.len();

        let exchange_ratio = txs.len() as f32 / (from_exchanges + to_exchanges).max(1) as f32;
        let in_out_ratio = incoming_txs.len() as f32 / outgoing_txs.len().max(1) as f32;

        let (interval_mean, interval_std, _interval_entropy) = utils::stats_from_intervals(tx_intervals);
        let (value_mean, value_median, value_std, value_max) = utils::stats_from_values(tx_values);

        Ok(Some(vec![
            txs.len() as f32,
            incoming_txs.len() as f32,
            outgoing_txs.len() as f32,
            txs.len() as f32 / lifetime_days.max(1) as f32,
            start_ts as f32,
            end_ts as f32,
            interval_mean as f32,
            interval_std as f32,
            // interval_entropy as f32,
            (from_addrs + to_addrs) as f32,
            addr_reuse as f32,
            in_out_ratio,
            exchange_ratio,
            value_mean as f32,
            value_median as f32,
            value_std as f32,
            value_max as f32,
            ingoing_volume as f32,
            outgoing_volume as f32,
        ]))
    }

    pub async fn classify(&self, address: &str) -> Result<WalletClassPrediction> {
        let features = self.get_features(address).await?
            .ok_or(anyhow!("Not enough transactions to classify"))?;

        let code = self.client.get_code(address).await?;
        let has_code = code != "0x" && code != "0x0";
        if has_code {
            return Ok(WalletClassPrediction {
                class: WalletClass::Contract,
                confidence: 1.0,
            });
        }

        // TODO: Figure out a way to run the model in rust directly, surely theres some crate for it right?
        Python::attach(|py| {
            let joblib = PyModule::import(py, "joblib")?;
            let np = PyModule::import(py, "numpy")?;

            let model = joblib.call_method1("load", ("rf_model.joblib",))?;

            let features = np.call_method1("array", (vec![features],))?;

            let pred = model.call_method1("predict_proba", (features,))?;
            let probs = pred.extract::<Vec<Vec<f64>>>()?;

            let class_idx = probs[0]
                .iter()
                .enumerate()
                // compare (class, probability) tuples by probability
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(idx, _)| idx)
                .unwrap();
            let confidence = probs[0][class_idx] as f32;

            let class = match class_idx {
                0 => WalletClass::Exchange { hot: false },
                1 => WalletClass::Exchange { hot: true },
                2 => WalletClass::MevBot,
                3 => WalletClass::Customer,
                _ => unreachable!(),
            };
            Ok(WalletClassPrediction { class, confidence })
        })
    }
}
