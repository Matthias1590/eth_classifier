use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let etherscan_client = Arc::new(etherscan::ClientBuilder::new().build()?);
    let classifier = Arc::new(classifier::WalletClassifier::new(Arc::clone(&etherscan_client)));

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
