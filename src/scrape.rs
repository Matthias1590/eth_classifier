mod etherscan;
mod wallet_classifier;
mod exchange_list;

use std::sync::Arc;

use crate::wallet_classifier::WalletClassifier;
use indicatif::{ProgressBar, ProgressStyle};
use csv::ReaderBuilder;

fn get_addresses(csv_path: &str, address_column: usize) -> anyhow::Result<Vec<String>> {
    let mut addresses = vec![];

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)?;

    for result in reader.records() {
        let record = result?;
        if let Some(addr) = record.get(address_column) {
            addresses.push(addr.to_owned());
        }
    }

    Ok(addresses)
}

async fn create_csv(classifier: Arc<WalletClassifier>) -> anyhow::Result<()> {
    let addresses = get_addresses("all_classified.csv", 0)?
        .into_iter()
        .collect::<Vec<_>>();

    let pb = ProgressBar::new(addresses.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("[{wide_bar:.cyan/blue}] {pos}/{len} ({elapsed}/{eta})")
        .unwrap()
        .progress_chars("#-"));

    let tasks = addresses
        .into_iter()
        .map(|addr| {
            let classifier = Arc::clone(&classifier);
            let pb = pb.clone();
            tokio::spawn(async move {
                let features = classifier.get_features(&addr).await?;
                pb.inc(1);
                anyhow::Ok((addr, features))
            })
        })
        .collect::<Vec<_>>();

    let mut writer = csv::Writer::from_path("all_features.csv")?;

    for task in tasks {
        let res = task.await;
        if res.is_err() {
            eprintln!("Joining failed: {:?}", res.err());
            pb.inc(1);
            continue;
        }
        let res = res.unwrap();
        if res.is_err() {
            eprintln!("Task failed: {:?}", res.err());
            pb.inc(1);
            continue;
        }
        let res = res.unwrap();
        let (address, features) = res;
        if let Some(features) = features {
            writer.write_field(address)?;
            writer.serialize(features)?;
        }
        writer.flush()?;
    }
    writer.flush()?;

    pb.finish();

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let etherscan_api_key = std::env::var("ETHERSCAN_API_KEY")
        .expect("ETHERSCAN_API_KEY must be set");
    let etherscan_client = Arc::new(etherscan::Client::new(etherscan_api_key));

    let classifier = Arc::new(WalletClassifier::new(Arc::clone(&etherscan_client)));

    create_csv(Arc::clone(&classifier)).await?;

    Ok(())
}
