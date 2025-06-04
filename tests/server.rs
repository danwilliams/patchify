//! Server integration tests.

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

mod common;



//		Packages

use crate::common::{client::*, server::*, utils::*};
use assert_json_diff::assert_json_eq;
use reqwest::StatusCode;
use rubedo::{
	crypto::Sha256Hash,
	sugar::s,
};
use semver::Version;
use serde_json::{Value as JsonValue, json};
use sha2::{Sha256, Digest as _};
use std::{
	fs::{File, self},
	io::Write as _,
};



//		Tests

#[cfg(test)]
mod endpoints {
	use super::*;
	
	//		get_ping															
	#[tokio::test]
	async fn get_ping() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, _, _, _, body) = request(
			format!("http://{address}/api/ping"),
			None,
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(body.as_ref(), b"");
	}
	
	//		get_latest															
	#[tokio::test]
	async fn get_latest() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/latest"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		let parsed  = serde_json::from_slice::<JsonValue>(&body).unwrap();
		let crafted = json!({
			"version": s!("1.1.0"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, Some(s!("application/json")));
		assert_eq!(content_len,  Some(crafted.to_string().len()));
		assert_eq!(verified,     Some(true));
		assert_json_eq!(parsed, crafted);
	}
	#[tokio::test]
	async fn get_latest__fail_signature_verification() {
		initialize();
		let other_public_key   = generate_new_private_key().verifying_key();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/latest"),
			Some(other_public_key),
		).await;
		let parsed  = serde_json::from_slice::<JsonValue>(&body).unwrap();
		let crafted = json!({
			"version": s!("1.1.0"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, Some(s!("application/json")));
		assert_eq!(content_len,  Some(crafted.to_string().len()));
		assert_eq!(verified,     Some(false));
		assert_json_eq!(parsed, crafted);
	}
	
	//		get_hashes_version													
	#[tokio::test]
	async fn get_hashes_version() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/hashes/0.2.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		let parsed  = serde_json::from_slice::<JsonValue>(&body).unwrap();
		let crafted = json!({
			"version": s!("0.2.0"),
			"hash":    s!("45fb074c75cfae708144969a1df5b33d845c95475a5ed69a60736b9391aac73b"),
		});
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, Some(s!("application/json")));
		assert_eq!(content_len,  Some(crafted.to_string().len()));
		assert_eq!(verified,     Some(true));
		assert_json_eq!(parsed, crafted);
	}
	#[tokio::test]
	async fn get_hashes_version__not_found() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/hashes/3.2.1"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::NOT_FOUND);
		assert_eq!(content_type,  Some(s!("text/plain; charset=utf-8")));
		assert_eq!(content_len,   Some(23));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Version 3.2.1 not found");
	}
	#[tokio::test]
	async fn get_hashes_version__invalid() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/hashes/invalid"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::BAD_REQUEST);
		assert_eq!(content_type,  Some(s!("text/plain; charset=utf-8")));
		assert_eq!(content_len,   Some(72));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Invalid URL: unexpected character 'i' while parsing major version number");
	}
	
	//		get_releases_version												
	#[tokio::test]
	async fn get_releases_version() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/releases/1.0.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(content_type,  Some(s!("application/octet-stream")));
		assert_eq!(content_len,   Some(3));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"foo");
	}
	#[tokio::test]
	async fn get_releases_version__medium_binary() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/releases/1.1.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(content_type,  Some(s!("application/octet-stream")));
		assert_eq!(content_len,   Some(5_120));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), [0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF].repeat(512));
	}
	#[tokio::test]
	async fn get_releases_version__large_binary() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/releases/0.2.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(content_type,  Some(s!("application/octet-stream")));
		assert_eq!(content_len,   Some(5_242_880));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), [0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF].repeat(0x0008_0000));
	}
	#[tokio::test]
	async fn get_releases_version__not_found() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/releases/4.5.6"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::NOT_FOUND);
		assert_eq!(content_type,  Some(s!("text/plain; charset=utf-8")));
		assert_eq!(content_len,   Some(23));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Version 4.5.6 not found");
	}
	#[tokio::test]
	async fn get_releases_version__invalid() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/releases/invalid"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::BAD_REQUEST);
		assert_eq!(content_type,  Some(s!("text/plain; charset=utf-8")));
		assert_eq!(content_len,   Some(72));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Invalid URL: unexpected character 'i' while parsing major version number");
	}
	#[tokio::test]
	async fn get_releases_version__missing() {
		initialize();
		let (address, releases_dir) = create_test_server().await;
		fs::remove_file(releases_dir.path().join("test-1.0.0")).unwrap();
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/releases/1.0.0"),
			Some(KEY.get().unwrap().verifying_key()),
		).await;
		assert_eq!(status,        StatusCode::INTERNAL_SERVER_ERROR);
		assert_eq!(content_type,  Some(s!("text/plain; charset=utf-8")));
		assert_eq!(content_len,   Some(20));
		assert_eq!(verified,      None);
		assert_eq!(body.as_ref(), b"Release file missing");
	}
}

#[cfg(test)]
mod scenarios {
	use super::*;
	
	//		download_and_verify_latest_release									
	#[tokio::test]
	async fn download_and_verify_latest_release() {
		initialize();
		let (address, _releases_dir) = create_test_server().await;
		let public_key = Some(KEY.get().unwrap().verifying_key());
		
		//		Get latest version												
		let latest = {
			let (status, content_type, content_len, verified, body) = request(
				format!("http://{address}/api/latest"),
				public_key,
			).await;
			let json:  JsonValue = serde_json::from_slice(&body).unwrap();
			let latest: Version  = json["version"].as_str().unwrap().parse().unwrap();
			assert_eq!(status,       StatusCode::OK);
			assert_eq!(content_type, Some(s!("application/json")));
			assert_eq!(content_len,  Some(19));
			assert_eq!(verified,     Some(true));
			assert_eq!(latest,       Version::new(1, 1, 0));
			latest
		};
		
		//		Download release file											
		let release_file = {
			let (status, content_type, content_len, verified, body) = request(
				format!("http://{address}/api/releases/{latest}"),
				public_key,
			).await;
			assert_eq!(status,       StatusCode::OK);
			assert_eq!(content_type, Some(s!("application/octet-stream")));
			assert_eq!(content_len,  Some(5_120));
			assert_eq!(verified,     None);
			body
		};
		
		//		Verify release file												
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/hashes/{latest}"),
			public_key,
		).await;
		let json:    JsonValue  = serde_json::from_slice(&body).unwrap();
		let version: Version    = json["version"].as_str().unwrap().parse().unwrap();
		let hash:    Sha256Hash = json["hash"].as_str().unwrap().parse().unwrap();
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, Some(s!("application/json")));
		assert_eq!(content_len,  Some(93));
		assert_eq!(verified,     Some(true));
		assert_eq!(version,      latest);
		assert_eq!(hash,         Sha256Hash::from(Sha256::digest(release_file)));
	}
	
	//		download_and_verify_release_with_hash_fail							
	#[tokio::test]
	async fn download_and_verify_release_with_hash_fail() {
		initialize();
		let (address, releases_dir) = create_test_server().await;
		let public_key = Some(KEY.get().unwrap().verifying_key());
		let wanted     = Version::new(1, 0, 0);
		let mut file   = File::create(releases_dir.path().join("test-1.0.0")).unwrap();
		write!(file, "invalid").unwrap();
		
		//		Download release file											
		let release_file = {
			let (status, content_type, content_len, verified, body) = request(
				format!("http://{address}/api/releases/{wanted}"),
				public_key,
			).await;
			assert_eq!(status,        StatusCode::OK);
			assert_eq!(content_type,  Some(s!("application/octet-stream")));
			assert_eq!(content_len,   Some(7));
			assert_eq!(verified,      None);
			assert_eq!(body.as_ref(), b"invalid");
			body
		};
		
		//		Verify release file												
		let (status, content_type, content_len, verified, body) = request(
			format!("http://{address}/api/hashes/{wanted}"),
			public_key,
		).await;
		let json:    JsonValue = serde_json::from_slice(&body).unwrap();
		let version: Version   = json["version"].as_str().unwrap().parse().unwrap();
		let hash:    String    = json["hash"].as_str().unwrap().to_owned();
		assert_eq!(status,       StatusCode::OK);
		assert_eq!(content_type, Some(s!("application/json")));
		assert_eq!(content_len,  Some(93));
		assert_eq!(verified,     Some(true));
		assert_eq!(version,      wanted);
		assert_ne!(hash,         hex::encode(Sha256::digest(release_file)));
	}
}


