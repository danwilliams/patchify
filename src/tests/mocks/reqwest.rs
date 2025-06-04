//! This module provides methods to create mock responses.
//! 
//! The `Updater` struct is responsible for managing application upgrades by
//! sending HTTP requests to the API server. This module mocks the critical
//! parts of the `reqwest` crate using `sham`, in order to test the `Updater`
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

use std::collections::HashMap;
use crate::common::utils::*;
use ed25519_dalek::Signer as _;
use reqwest::{StatusCode, IntoUrl};
use rubedo::{
	crypto::{SigningKey, VerifyingKey},
	sugar::s,
};
use sham::reqwest::{MockError, MockResponse, create_mock_response as create_sham_response};



//		Enums																											

//		ResponseSignature														
#[expect(variant_size_differences, reason = "Doesn't matter here")]
pub enum ResponseSignature {
	Generate,
	GenerateUsing(SigningKey),
	Omit,
	Use(String),
}



//		Functions																										

//		create_mock_response													
pub fn create_mock_response<U: IntoUrl, S: Into<String>>(
	url:          U,
	status:       StatusCode,
	content_type: Option<S>,
	content_len:  Option<usize>,
	body:         Result<&String, MockError>,
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
	let mut headers = HashMap::new();
	match *sign {
		ResponseSignature::GenerateUsing(_) |
		ResponseSignature::Generate         |
		ResponseSignature::Use(_)           => drop(headers.insert("X-Signature", signature)),
		ResponseSignature::Omit             => {},
	}
	let mock_response = create_sham_response(
		url,
		status,
		content_type,
		content_len,
		headers,
		match body {
			Ok(b)  => Ok(b.as_bytes()),
			Err(e) => Err(e),
		},
	);
	(mock_response, key.verifying_key())
}

//		create_mock_binary_response												
pub fn create_mock_binary_response<U: IntoUrl, S: Into<String>>(
	url:          U,
	status:       StatusCode,
	content_type: Option<S>,
	content_len:  Option<usize>,
	body:         Result<&[u8], MockError>,
) -> MockResponse {
	create_sham_response(
		url,
		status,
		content_type,
		content_len,
		HashMap::<String, String>::new(),
		body,
	)
}


