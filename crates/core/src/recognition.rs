//! Character candidates from an unauthenticated Metom image request.
use crate::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const METOM_ENDPOINT: &str = "https://mp.ex.nii.ac.jp/metom/api/predict";
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Prediction {
    pub character: String,
    pub probability: f64,
}
#[derive(Deserialize)]
struct Response {
    predictions: Vec<(String, f64)>,
}
pub async fn predict(image: &[u8]) -> Result<Vec<Prediction>> {
    predict_at(METOM_ENDPOINT, image).await
}
async fn predict_at(endpoint: &str, image: &[u8]) -> Result<Vec<Prediction>> {
    let response: Response = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .post(endpoint)
        .json(&serde_json::json!({"image_base64": STANDARD.encode(image), "k": 10, "return_probs": true}))
        .send().await?.error_for_status()?.json().await?;
    if response
        .predictions
        .iter()
        .any(|(c, p)| c.is_empty() || !p.is_finite() || !(0.0..=1.0).contains(p))
    {
        return Err(Error::Invalid("文字認識の候補を読み取れません。".into()));
    }
    Ok(response
        .predictions
        .into_iter()
        .take(10)
        .map(|(character, probability)| Prediction {
            character,
            probability,
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header_exists, method},
    };
    #[tokio::test]
    async fn exact_contract_without_authentication() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_json(
                serde_json::json!({"image_base64":"AQID", "k":10, "return_probs":true}),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"predictions":[["候",0.9],["𠮷",0.1]]})),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(header_exists("authorization"))
            .respond_with(ResponseTemplate::new(403))
            .expect(0)
            .mount(&server)
            .await;
        let result = predict_at(&server.uri(), &[1, 2, 3]).await?;
        assert_eq!(result[0].character, "候");
        assert_eq!(result[1].probability, 0.1);
        Ok(())
    }
    #[tokio::test]
    async fn rejects_non_json_and_invalid_probabilities() {
        let server = MockServer::start().await;
        for body in ["<html>unavailable</html>", r#"{"predictions":[["候",2]]}"#] {
            server.reset().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&server)
                .await;
            assert!(predict_at(&server.uri(), &[1]).await.is_err());
        }
    }
}
