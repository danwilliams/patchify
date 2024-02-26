#![allow(non_snake_case)]

//		Packages

use super::*;
use crate::mocks::*;
use assert_json_diff::assert_json_eq;
use claims::{assert_err_eq, assert_ok};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use reqwest::StatusCode;
use serde_json::{Value as JsonValue, json};
use tokio::fs;



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
		actions:     AtomicUsize::new(0),
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
mod updater_construction {
	use super::*;
	
	//		new																	
	#[tokio::test]
	async fn new() {
		let order   = Ordering::SeqCst;
		let updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              "https://api.example.com".parse().unwrap(),
			key:              VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			check_on_startup: false,
			check_interval:   Some(Duration::from_secs(60 * 60)),
		});
		assert_eq!(updater.actions.load(order),     0);
		assert_eq!(updater.config.version,          Version::new(1, 0, 0));
		assert_eq!(updater.config.api,              "https://api.example.com".parse().unwrap());
		assert_eq!(updater.config.key,              VerifyingKey::from_bytes(&[0; 32]).unwrap());
		assert_eq!(updater.config.check_on_startup, false);
		assert_eq!(updater.config.check_interval,   Some(Duration::from_secs(60 * 60)));
	}
}

#[cfg(test)]
mod updater_public {
	use super::*;
	
	//		register_action														
	#[tokio::test]
	async fn register_action() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.actions.load(order), 1);
		assert_eq!(updater.register_action(),   Some(2));
		assert_eq!(updater.actions.load(order), 2);
	}
	#[tokio::test]
	async fn register_action__overflow() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			MockClient::new(),
		);
		let _ = updater.actions.fetch_add(usize::MAX - 1, order);
		assert_eq!(updater.actions.load(order), usize::MAX - 1);
		assert_eq!(updater.register_action(),   Some(usize::MAX));
		assert_eq!(updater.actions.load(order), usize::MAX);
		assert_eq!(updater.register_action(),   None);
		assert_eq!(updater.actions.load(order), usize::MAX);
	}
	
	//		deregister_action													
	#[tokio::test]
	async fn deregister_action() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.register_action(),   Some(2));
		assert_eq!(updater.register_action(),   Some(3));
		assert_eq!(updater.actions.load(order), 3);
		assert_eq!(updater.deregister_action(), Some(2));
		assert_eq!(updater.deregister_action(), Some(1));
		assert_eq!(updater.actions.load(order), 1);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.actions.load(order), 0);
	}
	#[tokio::test]
	async fn deregister_action__underflow() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.actions.load(order), 1);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.deregister_action(), None);
		assert_eq!(updater.actions.load(order), 0);
	}
	
	//		is_safe_to_update													
	#[tokio::test]
	async fn is_safe_to_update() {
		let order   = Ordering::SeqCst;
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			MockClient::new(),
		);
		assert_eq!(updater.actions.load(order), 0);
		assert_eq!(updater.is_safe_to_update(), true);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.register_action(),   Some(2));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.deregister_action(), Some(1));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.is_safe_to_update(), true);
		assert_eq!(updater.register_action(),   Some(1));
		assert_eq!(updater.is_safe_to_update(), false);
		assert_eq!(updater.deregister_action(), Some(0));
		assert_eq!(updater.is_safe_to_update(), true);
	}
}

#[cfg(test)]
mod updater_private {
	use super::*;
	
	//		download_update														
	#[tokio::test]
	async fn download_update() {
		let version       = Version::new(2, 3, 4);
		let url           = "https://api.example.com/api/releases/2.3.4";
		let payload       = b"Test payload";
		let mock_response = create_mock_binary_response(
			StatusCode::OK,
			Some(s!("application/octet-stream")),
			Ok(payload),
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			mock_client,
		);
		let (_download_dir, update_path, file_hash) = updater.download_update(&version).await.unwrap();
		let file_data                               = fs::read(update_path).await.unwrap();
		assert_eq!(file_hash, Sha256::digest(payload).as_slice());
		assert_eq!(file_hash, Sha256::digest(&file_data).as_slice());
		assert_eq!(file_data, payload);
	}
	#[tokio::test]
	async fn download_update__err_unable_to_create_download() {
		//	No test for this at present, as it is difficult to simulate a failure
	}
	#[tokio::test]
	async fn download_update__err_unable_to_create_tempdir() {
		//	No test for this at present, as it is difficult to simulate a failure
	}
	#[tokio::test]
	async fn download_update__err_unable_to_write_to_download() {
		//	No test for this at present, as it is difficult to simulate a failure
	}
	#[tokio::test]
	async fn download_update__err_unexpected_content_type() {
		let version               = Version::new(2, 3, 4);
		let url                   = "https://api.example.com/api/releases/2.3.4";
		let content_type          = s!("text/plain");
		let expected_content_type = s!("application/octet-stream");
		let payload               = b"Test payload";
		let mock_response         = create_mock_binary_response(
			StatusCode::OK,
			Some(content_type.clone()),
			Ok(payload),
		);
		let mock_client = create_mock_client(
			url,
			Ok(mock_response),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			mock_client,
		);
		let err = updater.download_update(&version).await.unwrap_err();
		assert_eq!(err,             UpdaterError::UnexpectedContentType(url.parse().unwrap(), content_type.clone(), expected_content_type.clone()));
		assert_eq!(err.to_string(), format!(r#"HTTP response from {url} had unexpected content type: "{content_type}", expected: "{expected_content_type}""#));
	}
	
	//		verify_update														
	#[tokio::test]
	async fn verify_update() {
		let version                     = Version::new(2, 3, 4);
		let hash                        = Sha256::digest(b"Test payload");
		let url                         = "https://api.example.com/api/hashes/2.3.4";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": version.to_string(),
				"hash":    hex::encode(hash),
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
		assert_ok!(updater.verify_update(&version, hash.as_slice().try_into().unwrap()).await);
	}
	#[tokio::test]
	async fn verify_update__err_failed_hash_verification() {
		let version                     = Version::new(2, 3, 4);
		let hash                        = Sha256::digest(b"Test payload");
		let other_hash                  = Sha256::digest(b"Some other payload");
		let url                         = "https://api.example.com/api/hashes/2.3.4";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": version.to_string(),
				"hash":    hex::encode(other_hash),
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
		let err = updater.verify_update(&version, hash.as_slice().try_into().unwrap()).await;
		assert_err_eq!(err.clone(), UpdaterError::FailedHashVerification(version.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!("Failed hash verification for downloaded version {version}"));
	}
	#[tokio::test]
	async fn verify_update__err_invalid_payload() {
		let version                     = Version::new(2, 3, 4);
		let other_version               = Version::new(3, 3, 3);
		let hash                        = Sha256::digest(b"Test payload");
		let url                         = "https://api.example.com/api/hashes/2.3.4";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": other_version.to_string(),
				"hash":    hex::encode(hash),
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
		let err = updater.verify_update(&version, hash.as_slice().try_into().unwrap()).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidPayload(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid payload received from {url}"));
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
		let (url2, response)   = updater.request("latest").await.unwrap();
		let parsed:  JsonValue = serde_json::from_str(&response.text().await.unwrap()).unwrap();
		let crafted: JsonValue = json!({
			"version": s!("3.3.3"),
		});
		assert_eq!(url2.as_str(), url);
		assert_json_eq!(parsed, crafted);
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
		let err = updater.request("latest").await;
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
		let err = updater.request("latest").await;
		assert_err_eq!(err.clone(), UpdaterError::HttpRequestFailed(url.parse().unwrap(), err_msg.to_owned()));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP request to {url} failed: {err_msg}"));
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
		let err = updater.request(endpoint).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidUrl(base.parse().unwrap(), endpoint.to_owned()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid URL specified: {base} plus {endpoint}"));
	}
	
	//		decode_and_verify													
	#[tokio::test]
	async fn decode_and_verify__latest_version() {
		let version                     = Version::new(3, 3, 3);
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": version.to_string(),
			}).to_string()),
			ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let response = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await.unwrap();
		assert_eq!(response.version, version);
	}
	#[tokio::test]
	async fn decode_and_verify__version_hash() {
		let version                     = Version::new(3, 3, 3);
		let hash                        = hex::encode(Sha256::digest(b"Test payload"));
		let url                         = "https://api.example.com/api/hashes/3.3.3";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": version.to_string(),
				"hash":    hash,
			}).to_string()),
			ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let response = updater.decode_and_verify::<VersionHashResponse>(url.parse().unwrap(), mock_response).await.unwrap();
		assert_eq!(response.version, version);
		assert_eq!(response.hash,    hash);
	}
	#[tokio::test]
	async fn decode_and_verify__err_failed_signature_verification() {
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
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			other_public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::FailedSignatureVerification(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Failed signature verification for response from {url}"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_invalid_body() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Err(MockError {}),
			ResponseSignature::Use(s!("dummy signature")),
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidBody(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid HTTP body received from {url}"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_invalid_payload() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(s!("{invalid json: 3.3.3")),
			ResponseSignature::Generate,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidPayload(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("Invalid payload received from {url}"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_invalid_signature() {
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
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::InvalidSignature(url.parse().unwrap(), signature.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!(r#"Invalid signature header "{signature}" received from {url}"#));
	}
	#[tokio::test]
	async fn decode_and_verify__err_missing_signature() {
		let url                         = "https://api.example.com/api/latest";
		let (mock_response, public_key) = create_mock_response(
			StatusCode::OK,
			Some(s!("application/json")),
			Ok(json!({
				"version": s!("3.3.3"),
			}).to_string()),
			ResponseSignature::Omit,
		);
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::MissingSignature(url.parse().unwrap()));
		assert_eq!(err.unwrap_err().to_string(), format!("HTTP response from {url} does not contain a signature header"));
	}
	#[tokio::test]
	async fn decode_and_verify__err_unexpected_content_type() {
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
		let updater = setup_safe_updater(
			Version::new(1, 0, 0),
			"https://api.example.com/api/",
			public_key,
			MockClient::new(),
		);
		let err = updater.decode_and_verify::<LatestVersionResponse>(url.parse().unwrap(), mock_response).await;
		assert_err_eq!(err.clone(), UpdaterError::UnexpectedContentType(url.parse().unwrap(), content_type.clone(), expected_content_type.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!(r#"HTTP response from {url} had unexpected content type: "{content_type}", expected: "{expected_content_type}""#));
	}
}


