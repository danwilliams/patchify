#![allow(non_snake_case)]

//		Modules

#[allow(unused)]
mod common;



//		Packages

use crate::common::client::request;
use ed25519_dalek::VerifyingKey;
use patchify::client::{Config, Updater};
use reqwest::StatusCode;
use semver::Version;
use std::{
	io::{BufReader, BufRead},
	net::SocketAddr,
	process::{Command, Stdio},
	time::Duration,
};
use test_binary::build_test_binary;
use tokio::time::sleep;
use wiremock::{
	Mock,
	MockServer,
	ResponseTemplate,
	matchers::{method, path},
};



//		Tests

#[cfg(test)]
mod foundation {
	use super::*;
	
	//		ping_mock_server													
	#[tokio::test]
	async fn ping_mock_server() {
		let mock_server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/api/ping"))
			.respond_with(ResponseTemplate::new(200))
			.mount(&mock_server).await
		;
		let (status, _, _, body) = request(
			format!("{}/api/ping", mock_server.uri()),
			None,
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(body.as_ref(), b"");
	}
	
	//		ping_test_server													
	#[tokio::test]
	async fn ping_test_server() {
		let testbin_path = build_test_binary("standard-api-server", "testbins").unwrap();
		let mut subproc  = Command::new(testbin_path).stdout(Stdio::piped()).spawn().unwrap();
		let reader       = BufReader::new(subproc.stdout.take().unwrap());
		let mut address  = String::new();
		for line in reader.lines() {
			let line     = line.unwrap();
			if line.contains("Listening on") {
				address  = line.split_whitespace().last().unwrap().to_owned();
				break;
			}
		}
		if address.is_empty() {
			panic!("Server address not found in stdout");
		}
		let address: SocketAddr  = address.parse().unwrap();
		let (status, _, _, body) = request(
			format!("http://{address}/api/ping"),
			None,
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(body.as_ref(), b"");
		subproc.kill().unwrap();
	}
}

#[cfg(test)]
mod actions {
	use super::*;
	
	//		new																	
	#[tokio::test]
	async fn new__check_at_startup_only() {
		let mock_server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/api/latest"))
			.respond_with(ResponseTemplate::new(200))
			.expect(1)
			.mount(&mock_server).await
		;
		let _updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              format!("{}/api/", mock_server.uri()).parse().unwrap(),
			key:              VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			check_on_startup: true,
			check_interval:   None,
		}).unwrap();
		sleep(Duration::from_millis(50)).await;
	}
	#[tokio::test]
	async fn new__no_check_at_startup_only() {
		let mock_server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/api/latest"))
			.respond_with(ResponseTemplate::new(200))
			.expect(0)
			.mount(&mock_server).await
		;
		let _updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              format!("{}/api/", mock_server.uri()).parse().unwrap(),
			key:              VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			check_on_startup: false,
			check_interval:   None,
		}).unwrap();
		sleep(Duration::from_millis(100)).await;
	}
	#[tokio::test]
	async fn new__check_at_startup_and_at_intervals() {
		let mock_server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/api/latest"))
			.respond_with(ResponseTemplate::new(200))
			.expect(3)
			.mount(&mock_server).await
		;
		let _updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              format!("{}/api/", mock_server.uri()).parse().unwrap(),
			key:              VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			check_on_startup: true,
			check_interval:   Some(Duration::from_millis(50)),
		}).unwrap();
		sleep(Duration::from_millis(125)).await;
	}
	#[tokio::test]
	async fn new__no_check_on_startup_but_checks_at_intervals() {
		let mock_server = MockServer::start().await;
		Mock::given(method("GET"))
			.and(path("/api/latest"))
			.respond_with(ResponseTemplate::new(200))
			.expect(2)
			.mount(&mock_server).await
		;
		let _updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              format!("{}/api/", mock_server.uri()).parse().unwrap(),
			key:              VerifyingKey::from_bytes(&[0; 32]).unwrap(),
			check_on_startup: false,
			check_interval:   Some(Duration::from_millis(50)),
		}).unwrap();
		sleep(Duration::from_millis(125)).await;
	}
}


