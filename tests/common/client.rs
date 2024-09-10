//! Common shared client functionality for tests and examples.

//		Packages

use bytes::Bytes;
use ed25519_dalek::Signature;
use hex;
use reqwest::{Client, StatusCode, header::CONTENT_LENGTH, header::CONTENT_TYPE};
use rubedo::crypto::VerifyingKey;



//		Functions

//		request																	
/// Sends a GET request to the provided URL and returns the response.
/// 
/// Returns the status code, content type, content length, whether the response
/// signature was verified, and the response body.
/// 
/// # Parameters
/// 
/// * `url`        - The URL to send the request to.
/// * `public_key` - The public key to use for verifying the response signature.
/// 
pub async fn request(url: String, public_key: Option<VerifyingKey>) -> (StatusCode, Option<String>, Option<usize>, Option<bool>, Bytes) {
	let response     = Client::new().get(url).send().await.unwrap();
	let status       = response.status();
	let content_type = response.headers().get(CONTENT_TYPE)  .and_then(|h| h.to_str().ok()).map(ToOwned::to_owned);
	let content_len  = response.headers().get(CONTENT_LENGTH).and_then(|h| h.to_str().ok()).and_then(|s| s.parse::<usize>().ok());
	let signature    = response.headers().get("x-signature") .and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
	let body         = response.bytes().await.unwrap();
	let verified     = if public_key.is_none() || signature.is_empty() {
		None
	} else {
		let signature_bytes            = hex::decode(signature).unwrap();
		let signature_array: &[u8; 64] = signature_bytes.as_slice().try_into().unwrap();
		Some(public_key.unwrap().verify_strict(&body, &Signature::from_bytes(signature_array)).is_ok())
	};
	(status, content_type, content_len, verified, body)
}


