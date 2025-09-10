use anyhow::anyhow;
use pyo3::prelude::*;
use std::{fmt::Display, sync::Arc};
use tch::{Kind, Tensor};

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
                if *hot {
                    write!(f, "hot exchange")
                } else {
                    write!(f, "cold exchange")
                }
            }
            WalletClass::Contract => write!(f, "contract"),
        }
    }
}

pub struct WalletClassPrediction {
    pub class: WalletClass,
    pub confidence: f32,
}

fn stats_from_intervals(intervals: Vec<u64>) -> (f64, f64, f64) {
    let t = Tensor::from_slice(&intervals.iter().map(|&x| x as f64).collect::<Vec<_>>());

    let mean = t.mean(Kind::Float).double_value(&[]);
    let std = t.std(false).double_value(&[]); // biased = population std
    let probs = &t / t.sum(Kind::Float);
    let entropy = (-(&probs * probs.log())).sum(Kind::Float).double_value(&[]);

    (mean, std, entropy)
}

fn stats_from_values(values: Vec<f64>) -> (f64, f64, f64, f64) {
    let t = Tensor::from_slice(&values);

    let mean = t.mean(Kind::Float).double_value(&[]);
    let median = t.median().double_value(&[]);
    let std = t.std(false).double_value(&[]);
    let max = t.max().double_value(&[]);

    (mean, median, std, max)
}

pub struct WalletClassifier {
    client: Arc<crate::etherscan::Client>,
}

impl WalletClassifier {
    pub fn new(client: Arc<crate::etherscan::Client>) -> Self {
        Self { client }
    }

    // FIXME: Use libtorch more, all these cpu loops are a waste of time
    pub async fn get_features(&self, address: &str) -> anyhow::Result<Option<Vec<f32>>> {
        let address = address.to_lowercase();

        let txs = self.client.get_transactions(&address).await?;
        if txs.len() < 2 {
            return Ok(None);
        }

        // FIXME: This assumes the transactions are sorted by timestamp, only true because of an implementation detail in get_transactions, add sort parameter to get_transactions
        let start_ts = txs.last().expect("is_empty checked above")["timeStamp"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let end_ts = txs.first().expect("is_empty checked above")["timeStamp"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let lifetime_s = end_ts - start_ts;
        let lifetime_days = lifetime_s / (24 * 60 * 60);

        // FIXME: There's no error checking here, there shouldn't be either, get_transactions should parse json more strictly and return a vector of transaction structs
        let tx_intervals = txs
            .windows(2)
            .map(|w| {
                let end = w[0]["timeStamp"].as_str().unwrap().parse::<u64>().unwrap();
                let start = w[1]["timeStamp"].as_str().unwrap().parse::<u64>().unwrap();
                end - start
            })
            .collect::<Vec<_>>();

        let incoming_txs: Vec<_> = txs
            .iter()
            .filter(|tx| tx["to"].as_str().unwrap().to_lowercase() == address)
            .collect();
        let outgoing_txs: Vec<_> = txs
            .iter()
            .filter(|tx| tx["from"].as_str().unwrap().to_lowercase() == address)
            .collect();

        let from_exchanges = incoming_txs
            .iter()
            .filter(|tx| {
                let from = tx["from"].as_str().unwrap().to_lowercase();
                crate::exchange_list::is_exchange_owned(&from)
            })
            .count();
        let to_exchanges = outgoing_txs
            .iter()
            .filter(|tx| {
                let to = tx["to"].as_str().unwrap().to_lowercase();
                crate::exchange_list::is_exchange_owned(&to)
            })
            .count();

        let tx_values = txs
            .iter()
            .map(|tx| tx["value"].as_str().unwrap().parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        let ingoing_volume = incoming_txs
            .iter()
            .map(|tx| tx["value"].as_str().unwrap().parse::<f64>().unwrap())
            .sum::<f64>();
        let outgoing_volume = outgoing_txs
            .iter()
            .map(|tx| tx["value"].as_str().unwrap().parse::<f64>().unwrap())
            .sum::<f64>();

        let from_addrs = incoming_txs
            .iter()
            .map(|tx| tx["from"].as_str().unwrap().to_lowercase())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let to_addrs = outgoing_txs
            .iter()
            .map(|tx| tx["to"].as_str().unwrap().to_lowercase())
            .collect::<std::collections::HashSet<_>>()
            .len();
        let addr_reuse = (from_addrs + to_addrs) / txs.len();

        let exchange_ratio = txs.len() as f32 / (from_exchanges + to_exchanges).max(1) as f32;
        let in_out_ratio = incoming_txs.len() as f32 / outgoing_txs.len().max(1) as f32;

        let (interval_mean, interval_std, _interval_entropy) = stats_from_intervals(tx_intervals);
        let (value_mean, value_median, value_std, value_max) = stats_from_values(tx_values);

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

    pub async fn classify(&self, address: &str) -> anyhow::Result<WalletClassPrediction> {
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

        // FIXME: Not very efficient... run the model in rust directly
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
