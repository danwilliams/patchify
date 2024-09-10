//! This module mocks the `reqwest` crate in order to test the `Updater` struct.
//! 
//! The `Updater` struct is responsible for managing application upgrades by
//! sending HTTP requests to the API server. This module mocks the critical
//! parts of the `reqwest` crate using `mockall`, in order to test the `Updater`
//! struct without actually sending requests to the API server. This is
//! important because unit tests should not make actual network requests or rely
//! upon having a real server running.
//! 
//! The approach taken is that the "real" code imports the `Client` from
//! `reqwest` when running in non-test mode, but imports the mocked `Client`
//! when running in test mode. This is achieved by using conditional
//! compilation. The test code then configures the mocks to expect certain
//! requests and to return certain responses, and then runs the tests.
//! 

//		Packages

use crate::common::utils::*;
use bytes::Bytes;
use core::{
	fmt::{Display, self},
	pin::Pin,
};
use ed25519_dalek::Signer;
use futures_util::stream::{Stream, self};
use mockall::{Sequence, automock, concretize};
use reqwest::{
	IntoUrl,
	StatusCode,
	Url,
	header::{HeaderMap, CONTENT_LENGTH, CONTENT_TYPE},
};
use rubedo::{
	crypto::{SigningKey, VerifyingKey},
	sugar::s,
};
use std::sync::Arc;



//		Enums

//		ResponseSignature														
#[expect(variant_size_differences, reason = "Doesn't matter here")]
pub enum ResponseSignature {
	Generate,
	GenerateUsing(SigningKey),
	Omit,
	Use(String),
}



//		Traits

//§		Client																	
#[automock]
pub trait Client {
	//		get																	
	#[concretize]
	fn get<U: IntoUrl>(&self, url: U) -> Arc<MockRequestBuilder>;
}

//§		RequestBuilder															
#[automock]
pub trait RequestBuilder {
	//		send																
	async fn send(&self) -> Result<MockResponse, MockError>;
}



//		Structs

//		MockError																
#[derive(Clone, Debug)]
pub struct MockError;

//󰭅		Display																	
impl Display for MockError {
	//		fmt																	
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Mocked Reqwest error")
	}
}

//		MockResponse															
#[derive(Clone, Debug)]
pub struct MockResponse {
	pub status:  StatusCode,
	pub headers: HeaderMap,
	pub body:    Result<Arc<Bytes>, MockError>,
}

//󰭅		MockResponse															
#[expect(clippy::missing_const_for_fn, reason = "Needed for mock")]
#[expect(clippy::unused_async,         reason = "Also needed for mock")]
impl MockResponse {
	//		bytes																
	pub async fn bytes(&self) -> Result<Arc<Bytes>, MockError> {
		self.body.clone()
	}
	
	//		bytes_stream														
	pub fn bytes_stream(&self) -> Pin<Box<dyn Stream<Item = Result<Bytes, MockError>> + Send>> {
		#[expect(clippy::pattern_type_mismatch, reason = "Not resolvable")]
		match &self.body {
			Ok(ref bytes) => {
				let cloned_bytes = Arc::clone(bytes);
				Box::pin(stream::once(async move { Ok((*cloned_bytes).clone()) }))
			},
			Err(err) => {
				let cloned_err = err.clone();
				Box::pin(stream::once(async move { Err(cloned_err) }))
			},
		}
	}
	
	//		headers																
	pub fn headers(&self) -> &HeaderMap {
		&self.headers
	}
	
	//		status																
	pub fn status(&self) -> StatusCode {
		self.status
	}
	
	//		text																
	pub async fn text(&self) -> Result<Arc<String>, MockError> {
		self.bytes().await.map(|bytes| Arc::new(String::from_utf8(bytes.to_vec()).unwrap()))
	}
}



//		Functions

//		create_mock_client														
pub fn create_mock_client(responses: Vec<(&str, Result<MockResponse, MockError>)>) -> MockClient {
	let mut mock_client = MockClient::new();
	let mut sequence    = Sequence::new();
	for (mock_url, mock_response) in responses {
		let expected_url: Url = mock_url.parse().unwrap();
		let mut mock_request  = MockRequestBuilder::new();
		let _ = mock_request.expect_send()
			.times(1)
			.returning(move || mock_response.clone())
		;
		//	Wrap the mock request in an Arc so that it can be cloned
		let request = Arc::new(mock_request);
		let _ = mock_client.expect_get()
			.withf(move |url| url.as_str() == expected_url.as_str())
			.times(1)
			.in_sequence(&mut sequence)
			.returning(move |_| Arc::clone(&request))
		;
	}
	mock_client
}

//		create_mock_response													
pub fn create_mock_response(
	status:       StatusCode,
	content_type: Option<&str>,
	content_len:  Option<usize>,
	body:         Result<String, MockError>,
	sign:         &ResponseSignature,
) -> (MockResponse, VerifyingKey) {
	let key = match *sign {
		ResponseSignature::GenerateUsing(ref key) => key.clone(),
		ResponseSignature::Generate |
		ResponseSignature::Omit     |
		ResponseSignature::Use(_)   => generate_new_private_key(),
	};
	let signature = match *sign {
		ResponseSignature::GenerateUsing(_) |
		ResponseSignature::Generate           => {
			body.as_ref().map_or_else(|_| s!(""), |b| key.sign(b.as_ref()).to_string())
		},
		ResponseSignature::Omit               => s!(""),
		ResponseSignature::Use(ref other_sig) => other_sig.clone(),
	};
	let mock_response = MockResponse {
		status,
		headers: {
			let mut headers = HeaderMap::new();
			if let Some(ct) = content_type {
				drop(headers.insert(CONTENT_TYPE, ct.parse().unwrap()));
			}
			if let Some(cl) = content_len {
				drop(headers.insert(CONTENT_LENGTH, format!("{cl}").parse().unwrap()));
			}
			match *sign {
				ResponseSignature::GenerateUsing(_) |
				ResponseSignature::Generate         |
				ResponseSignature::Use(_)           => drop(headers.insert("X-Signature", signature.parse().unwrap())),
				ResponseSignature::Omit             => {},
			}
			headers
		},
		body:    body.map(|str| Arc::new(Bytes::copy_from_slice(str.as_bytes()))),
	};
	(mock_response, key.verifying_key())
}

//		create_mock_binary_response												
pub fn create_mock_binary_response(
	status:       StatusCode,
	content_type: Option<&str>,
	content_len:  Option<usize>,
	body:         Result<&[u8], MockError>,
) -> MockResponse {
	MockResponse {
		status,
		headers: {
			let mut headers = HeaderMap::new();
			if let Some(ct) = content_type {
				drop(headers.insert(CONTENT_TYPE, ct.parse().unwrap()));
			}
			if let Some(cl) = content_len {
				drop(headers.insert(CONTENT_LENGTH, format!("{cl}").parse().unwrap()));
			}
			headers
		},
		body:    body.map(|bytes| Arc::new(bytes.to_vec().into())),
	}
}


