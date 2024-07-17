use std::time::Duration;

use anyhow::anyhow;
use reqwest::{Client as HttpClient, Method, RequestBuilder, Response, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use tracing::debug;

async fn decode_response<T>(res: Response) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    match res.text().await {
        Ok(data) => {
            // log::debug!("response data>>> {:?}", data);
            let value: T = serde_json::from_str(&data)?;
            Ok(value)
        }
        Err(e) => {
            log::error!("{:?}", e);
            Err(anyhow::anyhow!("{e}"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    pub base_url: Url,
    pub http_client: HttpClient,
}
impl Client {
    pub fn new(rpc_url: String) -> Self {
        Client {
            base_url: Url::parse(&rpc_url).expect("could not parse base_url"),
            http_client: HttpClient::new(),
        }
    }
    async fn prep_req(&self, method: Method, url: Url) -> anyhow::Result<RequestBuilder> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());

        let req = self
            .http_client
            .request(method, url)
            .headers(headers)
            .timeout(Duration::from_secs(15));
        Ok(req)
    }

    fn get_url(&self, path: &str) -> Url {
        self.base_url.join(path).expect("could not parse url")
    }

    async fn req<T, U>(&self, path: &str, method: Method, body: U) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
        U: Serialize,
    {
        let res = match method {
            Method::GET => {
                self.prep_req(method, self.get_url(path))
                    .await?
                    .send()
                    .await?
            }
            Method::POST => {
                self.prep_req(method, self.get_url(path))
                    .await?
                    .json(&body)
                    .send()
                    .await?
            }
            _ => unimplemented!(),
        };

        if res.status() != 200 {
            return Err(anyhow!(
                "http error, status:{:?}, {:?}",
                res.status(),
                res.text().await?,
            ));
        }
        // debug!("req failed");
        // println!("req: {:?}", res);

        decode_response::<T>(res).await
    }

    pub async fn post<T, U>(&self, path: &str, param: U) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
        U: Serialize,
    {
        self.req(path, Method::POST, param).await
    }

    pub async fn get<T>(&self, path: &str) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
    {
        self.req(path, Method::GET, "").await
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Root<T> {
    pub jsonrpc: String,
    pub id: String,
    pub result: Option<T>,
}
