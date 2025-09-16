use tokio::time::{Duration, sleep};
use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde::de::{self, Deserializer};

fn string_to_u64<'a, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'a>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(de::Error::custom)
}

#[derive(Deserialize)]
pub struct Transaction {
    #[serde(rename = "timeStamp", deserialize_with = "string_to_u64")]
    pub timestamp: u64,
    pub to: String,
    pub from: String,
    #[serde(deserialize_with = "string_to_u64")]
    pub value: u64,
}

pub struct ClientBuilder {
    api_key: Option<String>,
    reqwest_client: Option<reqwest::Client>,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            api_key: None,
            reqwest_client: None,
        }
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_owned());
        self
    }

    pub fn with_reqwest_client(mut self, client: reqwest::Client) -> Self {
        self.reqwest_client = Some(client);
        self
    }

    pub fn build(self) -> Result<Client> {
        Ok(Client {
            api_key: self.api_key
                .or_else(|| std::env::var("ETHERSCAN_API_KEY").ok())
                .ok_or(anyhow!("ETHERSCAN_API_KEY must be set"))?,
            reqwest_client: self.reqwest_client
                .unwrap_or_else(|| reqwest::Client::new()),
        })
    }
}

pub struct Client {
    api_key: String,
    reqwest_client: reqwest::Client,
}

impl Client {
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
    ) -> Result<serde_json::Value> {
        let url = self.get_url(module, action, address);

        loop {
            let resp = self.reqwest_client.get(&url).send().await?;
            let value: serde_json::Value = serde_json::from_str(&resp.text().await?)
                .or(Err(anyhow!("Failed to parse response as json")))?;

            let is_error = value["status"].as_str().is_some_and(|v| v != "1");
            if is_error {
                let error_message = value["message"].as_str().unwrap_or("unknown error");
                let error_result = value["result"].as_str().unwrap_or("");
                if error_result.contains("rate limit") || error_message.contains("try again") {
                    sleep(Duration::from_secs(1)).await;
                    continue;
                }
                if error_result.contains("Invalid API Key") {
                    return Err(anyhow!("Invalid Etherscan API Key"));
                }
                if error_message.contains("No transactions found") {
                    return Ok(value);
                }
                return Err(anyhow!(
                    "Etherscan API error: {}\n{}\n{}",
                    error_message,
                    url,
                    value
                ));
            }

            return Ok(value);
        }
    }

    pub async fn get_transactions(&self, address: &str) -> Result<Vec<Transaction>> {
        let json = self.get("account", "txlist", address).await?;
        let json_array = json["result"]
            .as_array()
            .cloned()
            .ok_or(anyhow!("Wallet not found"))?;

        let parsed = serde_json::from_value(json_array.into())
            .or(Err(anyhow!("Failed to parse transactions")))?;

        Ok(parsed)
    }

    pub async fn get_code(&self, address: &str) -> Result<String> {
        let json = self.get("proxy", "eth_getCode", address).await?;

        Ok(json["result"]
            .as_str()
            .ok_or(anyhow!("Wallet not found"))?
            .to_owned())
    }
}
