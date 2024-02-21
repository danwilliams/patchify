//		Packages

use bytes::Bytes;
use ed25519_dalek::{Signature, VerifyingKey};
use hex;
use reqwest::{Client, StatusCode, header::CONTENT_TYPE};



//		Functions

//		request																	
pub async fn request(url: String, public_key: Option<VerifyingKey>) -> (StatusCode, String, Option<bool>, Bytes) {
	let response     = Client::new().get(url).send().await.unwrap();
	let status       = response.status();
	let content_type = response.headers().get(CONTENT_TYPE) .and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
	let signature    = response.headers().get("x-signature").and_then(|h| h.to_str().ok()).unwrap_or("").to_owned();
	let body         = response.bytes().await.unwrap();
	let verified     = if public_key.is_none() || signature.is_empty() {
		None
	} else {
		let signature_bytes            = hex::decode(signature).unwrap();
		let signature_array: &[u8; 64] = signature_bytes.as_slice().try_into().unwrap();
		Some(public_key.unwrap().verify_strict(&body, &Signature::from_bytes(signature_array)).is_ok())
	};
	(status, content_type, verified, body)
}


