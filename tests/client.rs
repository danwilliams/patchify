//! Client integration tests.

#![allow(unused_crate_dependencies, reason = "Creates a lot of noise")]

//	Lints specifically disabled for integration tests
#![allow(
	non_snake_case,
	unreachable_pub,
	clippy::arithmetic_side_effects,
	clippy::cast_lossless,
	clippy::cast_precision_loss,
	clippy::cognitive_complexity,
	clippy::default_numeric_fallback,
	clippy::exhaustive_enums,
	clippy::exhaustive_structs,
	clippy::expect_used,
	clippy::indexing_slicing,
	clippy::let_underscore_must_use,
	clippy::let_underscore_untyped,
	clippy::missing_assert_message,
	clippy::missing_panics_doc,
	clippy::mod_module_files,
	clippy::must_use_candidate,
	clippy::panic,
	clippy::print_stdout,
	clippy::tests_outside_test_module,
	clippy::too_many_lines,
	clippy::unwrap_in_result,
	clippy::unwrap_used,
	reason = "Not useful in tests"
)]



//		Modules																											

#[expect(unused, reason = "Shared test code")]
mod common;



//		Packages																										

use crate::common::{client::request, utils::*};
use core::{
	net::SocketAddr,
	time::Duration,
};
use ed25519_dalek::Signer as _;
use patchify::client::{Config, Status, Updater};
use reqwest::StatusCode;
use rubedo::{
	crypto::{Sha256Hash, VerifyingKey},
	std::{ByteSized as _, FileExt as _},
};
use semver::Version;
use serde_json::json;
use sha2::{Sha256, Digest as _};
use std::{
	env::current_exe,
	fs::{File, self},
	io::{BufReader, BufRead as _},
	path::PathBuf,
	process::{Command, Stdio},
	sync::LazyLock,
};
use tempfile::tempdir;
use test_binary::build_test_binary;
use tokio::time::sleep;
use wiremock::{
	Mock,
	MockServer,
	ResponseTemplate,
	matchers::{method, path},
};



//		Statics																											

static EMPTY_PUBLIC_KEY: LazyLock<VerifyingKey> = LazyLock::new(|| VerifyingKey::from_bytes([0; 32]));



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
		let (status, _, _, _, body) = request(
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
		for l in reader.lines() {
			let line     = l.unwrap();
			if line.contains("Listening on") {
				line.split_whitespace().last().unwrap().clone_into(&mut address);
				break;
			}
		}
		assert!(!address.is_empty(), "Server address not found in stdout");
		let addr: SocketAddr        = address.parse().unwrap();
		let (status, _, _, _, body) = request(
			format!("http://{addr}/api/ping"),
			None,
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(body.as_ref(), b"");
		subproc.kill().unwrap();
		_ = subproc.wait().unwrap();
	}
}

#[cfg(test)]
mod mock_actions {
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
		let _ = updater.register_action();
		sleep(Duration::from_millis(100)).await;
		//	We've registered a critical action, so the installation will be blocked,
		//	which is what we want here, so that we can check the status is correct
		assert_eq!(updater.status(), Status::PendingRestart(version.clone()));
		//	Assuming the update process ran correctly, we now need to rename the
		//	test binary back to its original name, so that the next test can run
		let path = current_exe().unwrap();
		#[expect(clippy::case_sensitive_file_extension_comparisons, reason = "Desired here")]
		if path.file_name().unwrap().to_str().unwrap().ends_with(".old") {
			fs::rename(path.clone(), path.with_extension("")).unwrap();
		}
	}
}

#[cfg(test)]
mod test_actions {
	use super::*;
	
	//		upgrade_app_v1_to_v2												
	#[expect(clippy::too_many_lines, reason = "Acceptable here")]
	#[tokio::test]
	async fn upgrade_app_v1_to_v2() {
		//		Build test binaries												
		let testserver_path = build_test_binary("e2e-apisrv-server",    "testbins").unwrap();
		let testapp_v1_path = build_test_binary("e2e-apisrv-srvapp-v1", "testbins").unwrap();
		let testapp_v2_path = build_test_binary("e2e-apisrv-srvapp-v2", "testbins").unwrap();
		//		Copy application binaries to releases directory					
		let releases_dir = tempdir().unwrap();
		let _ = fs::copy(&testapp_v1_path, releases_dir.path().join("test-1.0.0")).unwrap();
		let _ = fs::copy(&testapp_v2_path, releases_dir.path().join("test-2.0.0")).unwrap();
		//		Copy application v1 to execution directory						
		let exec_dir  = tempdir().unwrap();
		let exec_path = exec_dir.path().join("testapp");
		let _ = fs::copy(&testapp_v1_path, &exec_path).unwrap();
		//		Obtain SHA256 hashes of the release files						
		let testapp_v1_hash = File::hash::<Sha256Hash>(&PathBuf::from(testapp_v1_path)).unwrap().to_hex();
		let testapp_v2_hash = File::hash::<Sha256Hash>(&PathBuf::from(testapp_v2_path)).unwrap().to_hex();
		//		Start main API server											
		let mut subproc_srv = Command::new(testserver_path)
			.env("RELEASES", releases_dir.path())
			.env("VERSION1", testapp_v1_hash)
			.env("VERSION2", testapp_v2_hash)
			.stdout(Stdio::piped())
			.spawn().unwrap()
		;
		let (srv_address, public_key) = {
			let reader          = BufReader::new(subproc_srv.stdout.take().unwrap());
			let mut address     = None;
			let mut public_key  = None;
			for l in reader.lines() {
				let line        = l.unwrap();
				if line.contains("Listening on") {
					address     = Some(line.split_whitespace().last().unwrap().to_owned());
				} else if line.contains("Public key") {
					public_key  = Some(line.split_whitespace().last().unwrap().to_owned());
				}
				if address.is_some() && public_key.is_some() {
					break;
				}
			}
			assert!(address.is_some(), "Server address not found in stdout from main API serverr");
			(
				address.unwrap().parse::<SocketAddr>().unwrap(),
				VerifyingKey::from_hex(&public_key.unwrap()).unwrap(),
			)
		};
		//		Ping main API server											
		{
			let (status, _, _, _, body) = request(
				format!("http://{srv_address}/api/ping"),
				None,
			).await;
			assert_eq!(status,        StatusCode::OK);
			assert_eq!(body.as_ref(), b"");
		}
		//		Start app API server v1											
		let mut subproc_app = Command::new(exec_path)
			.env("API_PORT",   srv_address.port().to_string())
			.env("PUBLIC_KEY", public_key.to_hex())
			.stdout(Stdio::piped())
			.spawn().unwrap()
		;
		let mut reader = BufReader::new(subproc_app.stdout.take().unwrap());
		let app1_address: SocketAddr = {
			let mut address = String::new();
			loop {
				let mut line = String::new();
				let count    = reader.read_line(&mut line).unwrap();
				if count == 0 {
					break;
				}
				if line.contains("Listening on") {
					line.split_whitespace().last().unwrap().clone_into(&mut address);
					break;
				}
			}
			assert!(!address.is_empty(), "Server address not found in stdout from app API server");
			address
		}.parse().unwrap();
		//		Ping app API server												
		{
			let (status, _, _, _, body) = request(
				format!("http://{app1_address}/api/ping"),
				None,
			).await;
			assert_eq!(status,        StatusCode::OK);
			assert_eq!(body.as_ref(), b"");
		}
		//		Check app API server version									
		{
			let (status, _, _, _, body) = request(
				format!("http://{app1_address}/api/version"),
				None,
			).await;
			assert_eq!(status,        StatusCode::OK);
			assert_eq!(body.as_ref(), b"1.0.0");
		}
		//		Wait for app API server to restart								
		let app2_address: SocketAddr = {
			//	This part of the tests is a little hinky. Perhaps there is a better way
			//	to do it... at present the API server spins up, and then we start the
			//	client app (which is also an API server in this test scenario). The app
			//	is set to check for updates on startup, but our initial check above for
			//	v1 should always complete more quickly than the process of checking,
			//	downloading, verifying, installing, and restarting. After the initial
			//	check, we listen to the messages emitted until we encounter another
			//	"Listening on" message, which tells us a) that the application has
			//	restarted, and b) what port it is now using (as that is random every
			//	time). However, if something goes wrong then this test could hang, which
			//	is not ideal.
			let mut address = String::new();
			loop {
				let mut line = String::new();
				let count    = reader.read_line(&mut line).unwrap();
				if count == 0 {
					break;
				}
				if line.contains("Listening on") {
					line.split_whitespace().last().unwrap().clone_into(&mut address);
					break;
				}
			}
			assert!(!address.is_empty(), "Server address not found in stdout from app API server");
			address
		}.parse().unwrap();
		//		Check app API server version again once restarted				
		let (status, _, _, _, body) = request(
			format!("http://{app2_address}/api/version"),
			None,
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(body.as_ref(), b"2.0.0");
		//		Kill processes													
		subproc_app.kill().unwrap();
		subproc_srv.kill().unwrap();
		_ = subproc_app.wait().unwrap();
		_ = subproc_srv.wait().unwrap();
	}
}


