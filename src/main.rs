mod etherscan;
mod exchange_list;
mod wallet_classifier;

use std::sync::Arc;

use crate::wallet_classifier::WalletClassifier;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let etherscan_api_key = std::env::var("ETHERSCAN_API_KEY")
        .expect("ETHERSCAN_API_KEY must be set");
    let etherscan_client = Arc::new(etherscan::Client::new(etherscan_api_key));

    let classifier = Arc::new(WalletClassifier::new(Arc::clone(&etherscan_client)));

    let args = std::env::args().skip(1).collect::<Vec<_>>();

    println!("address\t\t\t\t\t\tclass");
    for arg in args {
        let prediction = classifier.classify(&arg).await;
        match prediction {
            Err(e) => {
                eprintln!("Error classifying {}: {}", arg, e);
            }
            Ok(prediction) => {
                println!(
                    "{}\t{} ({:.1}%)",
                    arg,
                    prediction.class,
                    prediction.confidence * 100.0
                );
            }
        }
    }

    Ok(())
}
