#![allow(non_snake_case)]

//		Packages

use super::*;
use assert_json_diff::assert_json_eq;
use claims::{assert_err_eq, assert_none};
use rand::rngs::OsRng;
use rubedo::{
	http::{ResponseExt, UnpackedResponse},
	sugar::s,
};
use serde_json::json;
use sha2::{Sha256, Digest};
use std::{
	fs,
	io::Write,
};
use tempfile::{TempDir, tempdir};
use velcro::hash_map;



//		Constants

const VERSION_DATA: [(Version, usize, &[u8]); 5] = [
	(Version::new(1, 0, 0),       1, b"foo"),
	(Version::new(0, 1, 0),       1, b"bar"),
	(Version::new(0, 0, 1),       1, b"foobarbaz"),
	(Version::new(1, 1, 0),     512, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5KB binary string
	(Version::new(0, 2, 0), 524_288, &[0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF]),  //  5MB binary string
];



//		Common

//		setup_core																
fn setup_core(releases_dir: &TempDir) -> Result<Core, ReleaseError> {
	let mut csprng = OsRng{};
	Core::new(Config {
		appname:  s!("test"),
		key:      SigningKey::generate(&mut csprng),
		releases: releases_dir.path().to_path_buf(),
		versions: VERSION_DATA.iter()
			.map(|(version, repetitions, data)| (version.clone(), Sha256::digest(data.repeat(*repetitions)).into()))
			.collect()
		,
		stream_threshold: 1000,
		stream_buffer:    256,
		read_buffer:      128,
	})
}

//		setup_files																
fn setup_files() -> TempDir {
	let releases_dir = tempdir().unwrap();
	for (version, repetitions, data) in VERSION_DATA.iter() {
		let path     = releases_dir.path().join(&format!("test-{}", version));
		let mut file = File::create(&path).unwrap();
		file.write_all(&data.repeat(*repetitions)).unwrap();
	}
	releases_dir
}



//		Tests

//		Core																	
#[cfg(test)]
mod core {
	use super::*;
	
	//		new																	
	#[test]
	fn new() {
		let core = setup_core(&setup_files()).unwrap();
		assert_eq!(core.config.appname, "test");
		assert_eq!(core.latest,         Version::new(1, 1, 0));
	}
	#[test]
	fn new__err_missing() {
		let dir  = setup_files();
		let path = dir.path().join("test-1.0.0");
		fs::remove_file(&path).unwrap();
		let err  = setup_core(&dir);
		assert_err_eq!(err.clone(), ReleaseError::Missing(Version::new(1, 0, 0), path.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!("The release file for version 1.0.0 is missing: {path:?}"));
	}
	#[test]
	fn new__err_invalid() {
		let dir      = setup_files();
		let path     = dir.path().join("test-1.0.0");
		let mut file = File::create(&path).unwrap();
		write!(file, "invalid").unwrap();
		let err      = setup_core(&dir);
		assert_err_eq!(err.clone(), ReleaseError::Invalid(Version::new(1, 0, 0), path.clone()));
		assert_eq!(err.unwrap_err().to_string(), format!("The release file for version 1.0.0 failed hash verification: {path:?}"));
	}
	
	//		latest_version														
	#[test]
	fn latest_version() {
		let core = setup_core(&setup_files()).unwrap();
		assert_eq!(core.latest_version(), Version::new(1, 1, 0));
	}
	#[test]
	fn latest_version__empty() {
		let mut csprng = OsRng{};
		let core       = Core::new(Config {
			appname:  s!("test"),
			key:      SigningKey::generate(&mut csprng),
			releases: tempdir().unwrap().path().to_path_buf(),
			versions: hash_map!{},
			stream_threshold: 1000,
			stream_buffer:    256,
			read_buffer:      128,
		}).unwrap();
		assert_eq!(core.latest_version(), Version::new(0, 0, 0));
	}
	
	//		versions															
	#[test]
	fn versions() {
		let core = setup_core(&setup_files()).unwrap();
		assert_eq!(core.versions().iter()
			.map(|(version, hash)| (version.clone(), hex::encode(hash)))
			.collect::<HashMap<Version, String>>()
		, hash_map!{
			Version::new(1, 0, 0): s!("2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"),
			Version::new(0, 1, 0): s!("fcde2b2edba56bf408601fb721fe9b5c338d10ee429ea04fae5511b68fbf8fb9"),
			Version::new(0, 0, 1): s!("97df3588b5a3f24babc3851b372f0ba71a9dcdded43b14b9d06961bfc1707d9d"),
			Version::new(1, 1, 0): s!("71b9dacf6c68a207b01c2b05f6362e62c267cc86123a596821366f6753bf10fa"),
			Version::new(0, 2, 0): s!("45fb074c75cfae708144969a1df5b33d845c95475a5ed69a60736b9391aac73b"),
		});
	}
	#[test]
	fn versions__empty() {
		let mut csprng = OsRng{};
		let core       = Core::new(Config {
			appname:  s!("test"),
			key:      SigningKey::generate(&mut csprng),
			releases: tempdir().unwrap().path().to_path_buf(),
			versions: hash_map!{},
			stream_threshold: 1000,
			stream_buffer:    256,
			read_buffer:      128,
		}).unwrap();
		assert_eq!(core.versions(), hash_map!{});
	}
	
	//		release_file														
	#[test]
	fn release_file() {
		let core = setup_core(&setup_files()).unwrap();
		assert_eq!(core.release_file(&Version::new(1, 1, 0)).unwrap(), core.config.releases.join("test-1.1.0"));
	}
	#[test]
	fn release_file__not_found() {
		let core = setup_core(&setup_files()).unwrap();
		assert_none!(core.release_file(&Version::new(8, 7, 6)));
	}
}

//		Axum																	
#[cfg(test)]
mod axum {
	use super::*;
	
	//		get_latest_version													
	#[tokio::test]
	async fn get_latest_version() {
		let core     = Arc::new(setup_core(&setup_files()).unwrap());
		let unpacked = Axum::get_latest_version(Extension(core.clone())).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				//	Axum automatically adds a content-type header.
				(s!("content-type"), s!("application/json")),
				(s!("x-signature"),  core.config.key.sign(unpacked.body.as_ref()).to_string()),
			],
			json!({
				"version": s!("1.1.0"),
			}),
		);
		assert_json_eq!(unpacked, crafted);
	}
	
	//		get_hash_for_version												
	#[tokio::test]
	async fn get_hash_for_version() {
		let core     = Arc::new(setup_core(&setup_files()).unwrap());
		let unpacked = Axum::get_hash_for_version(
			Extension(core.clone()),
			Path(Version::new(0, 2, 0)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				//	Axum automatically adds a content-type header.
				(s!("content-type"), s!("application/json")),
				(s!("x-signature"),  core.config.key.sign(unpacked.body.as_ref()).to_string()),
			],
			json!({
				"version": s!("0.2.0"),
				"hash":    s!("45fb074c75cfae708144969a1df5b33d845c95475a5ed69a60736b9391aac73b"),
			}),
		);
		assert_json_eq!(unpacked, crafted);
	}
	#[tokio::test]
	async fn get_hash_for_version__not_found() {
		let core     = Arc::new(setup_core(&setup_files()).unwrap());
		let unpacked = Axum::get_hash_for_version(
			Extension(core),
			Path(Version::new(3, 2, 1)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::NOT_FOUND,
			vec![
				//	Axum automatically adds a content-type header.
				(s!("content-type"), s!("text/plain; charset=utf-8")),
			],
			"Version 3.2.1 not found",
		);
		assert_json_eq!(unpacked, crafted);
	}
	
	//		get_release_file													
	#[tokio::test]
	async fn get_release_file() {
		let dir      = setup_files();
		let core     = Arc::new(setup_core(&dir).unwrap());
		let unpacked = Axum::get_release_file(
			Extension(core.clone()),
			Path(Version::new(0, 0, 1)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				(s!("content-type"), s!("application/octet-stream")),
			],
			b"foobarbaz",
		);
		assert_json_eq!(unpacked, crafted);
	}
	#[tokio::test]
	async fn get_release_file__medium_binary() {
		let dir      = setup_files();
		let core     = Arc::new(setup_core(&dir).unwrap());
		let unpacked = Axum::get_release_file(
			Extension(core.clone()),
			Path(Version::new(1, 1, 0)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				(s!("content-type"), s!("application/octet-stream")),
			],
			vec![0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF].repeat(512),
		);
		assert_json_eq!(unpacked, crafted);
	}
	#[tokio::test]
	async fn get_release_file__large_binary() {
		let dir      = setup_files();
		let core     = Arc::new(setup_core(&dir).unwrap());
		let unpacked = Axum::get_release_file(
			Extension(core.clone()),
			Path(Version::new(0, 2, 0)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				(s!("content-type"), s!("application/octet-stream")),
			],
			vec![0x00, 0x01, 0x23, 0x45, 0x67, 0x89, 0x1A, 0xBC, 0xDE, 0xFF].repeat(524_288),
		);
		assert_json_eq!(unpacked, crafted);
	}
	#[tokio::test]
	async fn get_release_file__not_found() {
		let dir      = setup_files();
		let core     = Arc::new(setup_core(&dir).unwrap());
		let unpacked = Axum::get_release_file(
			Extension(core.clone()),
			Path(Version::new(7, 8, 9)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::NOT_FOUND,
			vec![
				//	Axum automatically adds a content-type header.
				(s!("content-type"), s!("text/plain; charset=utf-8")),
			],
			"Version 7.8.9 not found",
		);
		assert_json_eq!(unpacked, crafted);
	}
	#[tokio::test]
	async fn get_release_file__missing() {
		let dir      = setup_files();
		let core     = Arc::new(setup_core(&dir).unwrap());
		fs::remove_file(&dir.path().join("test-0.0.1")).unwrap();
		let unpacked = Axum::get_release_file(
			Extension(core.clone()),
			Path(Version::new(0, 0, 1)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::INTERNAL_SERVER_ERROR,
			vec![
				//	Axum automatically adds a content-type header.
				(s!("content-type"), s!("text/plain; charset=utf-8")),
			],
			"Release file missing",
		);
		assert_json_eq!(unpacked, crafted);
	}
	
	//		sign_response														
	#[test]
	fn sign_response() {
		let core     = Arc::new(setup_core(&setup_files()).unwrap());
		let unpacked = Axum::sign_response(&core.config.key.clone(), Response::builder()
			.status(StatusCode::OK)
			.body(Body::from(s!("This is a test")))
			.unwrap()
			.into_response()
		).unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				(s!("x-signature"), core.config.key.sign("This is a test".as_bytes()).to_string()),
			],
			"This is a test",
		);
		assert_json_eq!(unpacked, crafted);
	}
	#[test]
	fn sign_response__specific_key() {
		let mut csprng = OsRng{};
		let other_key  = SigningKey::generate(&mut csprng);
		let core       = Arc::new(setup_core(&setup_files()).unwrap());
		let unpacked   = Axum::sign_response(&core.config.key, Response::builder()
			.status(StatusCode::OK)
			.body(Body::from(s!("This is a test")))
			.unwrap()
			.into_response()
		).unpack().unwrap();
		assert_eq!(unpacked.status, StatusCode::OK);
		assert_eq!(unpacked.headers[0].name,  "x-signature");
		assert_eq!(unpacked.headers[0].value, core.config.key.sign("This is a test".as_bytes()).to_string());
		assert_ne!(unpacked.headers[0].value, other_key      .sign("This is a test".as_bytes()).to_string());
		assert_eq!(unpacked.body.as_bytes(),  b"This is a test");
	}
}


