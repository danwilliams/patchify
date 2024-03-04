#![allow(non_snake_case)]

//		Modules

#[allow(unused)]
mod common;



//		Packages

use crate::common::{client::request, utils::*};
use ed25519_dalek::{Signer, VerifyingKey};
use once_cell::sync::Lazy;
use patchify::client::{Config, Status, Updater};
use reqwest::StatusCode;
use semver::Version;
use serde_json::json;
use sha2::{Sha256, Digest};
use std::{
	env::current_exe,
	fs,
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



//		Constants

const EMPTY_PUBLIC_KEY: Lazy<VerifyingKey> = Lazy::new(|| VerifyingKey::from_bytes(&[0; 32]).unwrap());



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
			key:              *EMPTY_PUBLIC_KEY,
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
			key:              *EMPTY_PUBLIC_KEY,
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
			key:              *EMPTY_PUBLIC_KEY,
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
			key:              *EMPTY_PUBLIC_KEY,
			check_on_startup: false,
			check_interval:   Some(Duration::from_millis(50)),
		}).unwrap();
		sleep(Duration::from_millis(125)).await;
	}
	
	//		check_for_updates													
	#[tokio::test]
	async fn check_for_updates__complete_successful_process() {
		//	No test for this at present, as we can't restart from within the test
		//	environment. This is tested using the testbins full integration tests,
		//	and for the "normal" integration tests the behaviour up to the point of
		//	restart is tested.
	}
	#[tokio::test]
	async fn check_for_updates__no_update_available() {
		let mock_server = MockServer::start().await;
		let private_key = generate_new_private_key();
		let json_data   = json!({
			"version": "1.0.0",
		});
		Mock::given(method("GET"))
			.and(path("/api/latest"))
			.respond_with(
				ResponseTemplate::new(200)
					.append_header("Content-Type", "application/json")
					.append_header("X-Signature",  private_key.sign(json_data.to_string().as_ref()).to_string())
					.set_body_json(json_data)
			)
			.expect(1)
			.mount(&mock_server).await
		;
		let updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              format!("{}/api/", mock_server.uri()).parse().unwrap(),
			key:              private_key.verifying_key(),
			check_on_startup: true,
			check_interval:   None,
		}).unwrap();
		sleep(Duration::from_millis(50)).await;
		//	TODO: Should gain more insight into the outcome at some point, through
		//	TODO: error status or similar
		assert_eq!(updater.status(), Status::Idle);
	}
	#[tokio::test]
	async fn check_for_updates__restart_blocked() {
		let mock_server = MockServer::start().await;
		let version     = Version::new(2, 3, 4);
		let private_key = generate_new_private_key();
		let payload     = b"Test payload";
		let json_data1  = json!({
			"version": version,
		});
		let json_data2  = json!({
			"version": version,
			"hash":    hex::encode(Sha256::digest(payload)),
		});
		Mock::given(method("GET"))
			.and(path("/api/latest"))
			.respond_with(
				ResponseTemplate::new(200)
					.append_header("Content-Type", "application/json")
					.append_header("X-Signature",  private_key.sign(json_data1.to_string().as_ref()).to_string())
					.set_body_json(json_data1)
					//	Delay slightly to allow registration of the critical action
					.set_delay(Duration::from_millis(1))
			)
			.expect(1)
			.mount(&mock_server).await
		;
		Mock::given(method("GET"))
			.and(path(format!("/api/releases/{version}")))
			.respond_with(
				ResponseTemplate::new(200)
					.append_header("Content-Type", "application/octet-stream")
					.set_body_bytes(payload.to_vec())
			)
			.expect(1)
			.mount(&mock_server).await
		;
		Mock::given(method("GET"))
			.and(path(format!("/api/hashes/{version}")))
			.respond_with(
				ResponseTemplate::new(200)
					.append_header("Content-Type", "application/json")
					.append_header("X-Signature",  private_key.sign(json_data2.to_string().as_ref()).to_string())
					.set_body_json(json_data2)
			)
			.expect(1)
			.mount(&mock_server).await
		;
		let updater = Updater::new(Config {
			version:          Version::new(1, 0, 0),
			api:              format!("{}/api/", mock_server.uri()).parse().unwrap(),
			key:              private_key.verifying_key(),
			check_on_startup: true,
			check_interval:   None,
		}).unwrap();
		updater.register_action();
		sleep(Duration::from_millis(100)).await;
		//	We've registered a critical action, so the installation will be blocked,
		//	which is what we want here, so that we can check the status is correct
		assert_eq!(updater.status(), Status::PendingRestart(version.clone()));
		//	Assuming the update process ran correctly, we now need to rename the
		//	test binary back to its original name, so that the next test can run
		let path = current_exe().unwrap();
		if path.file_name().unwrap().to_str().unwrap().ends_with(".old") {
			fs::rename(path.clone(), path.with_extension("")).unwrap();
		}
	}
}


