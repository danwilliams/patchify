#![allow(non_snake_case)]

//		Packages

use super::*;
use crate::mocks::*;
use claims::assert_err_eq;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use reqwest::StatusCode;
use serde_json::json;



//		Common

//		setup_safe_updater														
///	This function sets up a safe `Updater` instance for testing.
/// 
/// The `Updater` instance is created by bypassing `Updater::new()`, so that the
/// real checks are not triggered. Additionally, it is created with a mock
/// `Client` instance, so that no actual network requests will be made.
/// 
fn setup_safe_updater(
	version:     Version,
	api:         &str,
	key:         VerifyingKey,
	mock_client: MockClient,
) -> Updater {
	let api         = api.parse().unwrap();
	//	This is needed for creation, but won't be used in tests
	let (sender, _) = flume::unbounded();
	//	The Updater instance needs to be created manually in order to bypass the
	//	actions performed in the new() method
	Updater {
		config:      Config {
			version,
			api,
			key,
			check_on_startup: false,
			check_interval:   None,
		},
		http_client: mock_client,
		queue:       sender,
	}
}



//		Tests

//		Updater																	
#[cfg(test)]
mod updater {
	use super::*;
	
	//		new																	
	#[tokio::test]
	async fn new() {
		let updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              "https://api.example.com".parse().unwrap(),
			key:              VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			check_on_startup: false,
			check_interval:   Some(Duration::from_secs(60 * 60)),
		});
		assert_eq!(updater.config.version,          Version::new(1, 0, 0));
		assert_eq!(updater.config.api,              "https://api.example.com".parse().unwrap());
		assert_eq!(updater.config.key,              VerifyingKey::from_bytes(&[0; 32]).unwrap());
		assert_eq!(updater.config.check_on_startup, false);
		assert_eq!(updater.config.check_interval,   Some(Duration::from_secs(60 * 60)));
	}
	
	//		request																
	#[tokio::test]
	async fn request() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": s!("3.3.3"),
			}).to_string()),
			ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let response = updater.request::<LatestVersionResponse>("latest").await.unwrap();
		assert_eq!(response.version, Version::new(3, 3, 3));
	}
	#[tokio::test]
	async fn request__err_failed_signature_verification() {
		let url                          = "https://api.example.com/api/latest";
		let mut csprng                   = OsRng{};
		let other_public_key             = SigningKey::generate(&mut csprng).verifying_key();
		let (mock_response, _public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": s!("3.3.3"),
			}).to_string()),
			ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			other_public_key,
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::FailedSignatureVerification(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Failed signature verification for response from {url}"));
	}
	#[tokio::test]
	async fn request__err_http_error() {
		let url                         = "https://api.example.com/api/latest";
		let status                      = StatusCode::IM_A_TEAPOT;
		let (mock_response, public_key) = create_mock_response(
			status,
			Some(s!("application/json")),
			Ok(json!({
				"version": s!("3.3.3"),
			}).to_string()),
			ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::HttpError(url.parse().unwrap(), status));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP status code {status} received when calling {url}"));
	}
	#[tokio::test]
	async fn request__err_http_request_failed() {
		let url         = "https://api.example.com/api/latest";
		let err_msg     = "Mocked Reqwest error";
		let mock_client = create_mock_client(
			url,
			Err(MockError {}),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::HttpRequestFailed(url.parse().unwrap(), err_msg.to_owned()));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP request to {url} failed: {err_msg}"));
	}
	#[tokio::test]
	async fn request__err_invalid_body() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Err(MockError {}),
			ResponseSignature::Use(s!("dummy signature")),
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidBody(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid HTTP body received from {url}"));
	}
	#[tokio::test]
	async fn request__err_invalid_payload() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(s!("{invalid json: 3.3.3")),
			ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidPayload(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid payload received from {url}"));
	}
	#[tokio::test]
	async fn request__err_invalid_signature() {
		let url                         = "https://api.example.com/api/latest";
		let signature                   = s!("invalid signature");
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": s!("3.3.3"),
			}).to_string()),
			ResponseSignature::Use(signature.clone()),
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidSignature(url.parse().unwrap(), signature.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!(r#"Invalid signature header "{signature}" received from {url}"#));
	}
	#[tokio::test]
	async fn request__err_invalid_url() {
		let base     = "https://api.example.com/api";
		let endpoint = "https://[invalid]/../../../endpoint";
		let updater  = setup_safe_updater(
			Version::new(1, 0, 0),
			base,
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			MockClient::new(),
		);
		let err = updater.request::<LatestVersionResponse>(endpoint).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidUrl(base.parse().unwrap(), endpoint.to_owned()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid URL specified: {base} plus {endpoint}"));
	}
	#[tokio::test]
	async fn request__err_missing_signature() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": s!("3.3.3"),
			}).to_string()),
			ResponseSignature::Omit,
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::MissingSignature(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP response from {url} does not contain a signature header"));
	}
	#[tokio::test]
	async fn request__err_unexpected_content_type() {
		let url                         = "https://api.example.com/api/latest";
		let content_type                = s!("text/plain");
		let expected_content_type       = s!("application/json");
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(content_type.clone()),
			Ok(json!({
				"version": s!("3.3.3"),
			}).to_string()),
			ResponseSignature::Generate,
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			mock_client,
		);
		let err = updater.request::<LatestVersionResponse>("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::UnexpectedContentType(url.parse().unwrap(), content_type.clone(), expected_content_type.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!(r#"HTTP response from {url} had unexpected content type: "{content_type}", expected: "{expected_content_type}""#));
	}
}


