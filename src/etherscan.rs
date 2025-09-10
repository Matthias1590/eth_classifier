use serde_json::error;
use tokio::time::{Duration, sleep};

pub struct Client {
    api_key: String,
    reqwest_client: reqwest::Client,
}

impl Client {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            reqwest_client: reqwest::Client::new(),
        }
    }

    fn get_url(&self, module: &str, action: &str, address: &str) -> String {
        format!(
            "https://api.etherscan.io/v2/api?chainid=1&module={}&action={}&address={}&sort=desc&apikey={}",
            module, action, address, self.api_key
        )
    }

    async fn get(
        &self,
        module: &str,
        action: &str,
        address: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let url = self.get_url(module, action, address);

        loop {
            let resp = self.reqwest_client.get(&url).send().await?;
            let value: serde_json::Value = serde_json::from_str(&resp.text().await?)
                .or(Err(anyhow::anyhow!("Failed to parse response as json")))?;

            let is_error = value["status"].as_str().is_some_and(|v| v != "1");
            if is_error {
                let error_message = value["message"].as_str().unwrap_or("unknown error");
                let error_result = value["result"].as_str().unwrap_or("");
                if error_result.contains("rate limit") || error_message.contains("try again") {
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                if error_result.contains("Invalid API Key") {
                    return Err(anyhow::anyhow!("Invalid Etherscan API Key"));
                }
                if error_message.contains("No transactions found") {
                    return Ok(value);
                }
                return Err(anyhow::anyhow!(
                    "Etherscan API error: {}\n{}\n{}",
                    error_message,
                    url,
                    value
                ));
            }

            return Ok(value);
        }
    }

    pub async fn get_transactions(&self, address: &str) -> anyhow::Result<Vec<serde_json::Value>> {
        let json = self.get("account", "txlist", address).await?;

        // FIXME: This clone probably has some impact, I'm just too lazy to use ["result"].as_array() everywhere else
        Ok(json["result"]
            .as_array()
            .cloned()
            .ok_or(anyhow::anyhow!("Wallet not found"))?)
    }

    pub async fn get_code(&self, address: &str) -> anyhow::Result<String> {
        let json = self.get("proxy", "eth_getCode", address).await?;

        Ok(json["result"]
            .as_str()
            .ok_or(anyhow::anyhow!("Wallet not found"))?
            .to_owned())
    }
}
