#![allow(non_snake_case)]

//		Packages

use super::*;
use assert_json_diff::assert_json_eq;
use claims::assert_err_eq;
use rubedo::{
	http::{ResponseExt, UnpackedResponse},
	sugar::s,
};
use sha2::{Sha256, Digest};
use std::{
	fs,
	io::Write,
};
use tempfile::{TempDir, tempdir};
use velcro::hash_map;



//		Constants

const VERSION_DATA: [(Version, &str); 5] = [
	(Version::new(1, 0, 0), "foo"),
	(Version::new(0, 1, 0), "bar"),
	(Version::new(0, 0, 1), "baz"),
	(Version::new(1, 1, 0), "foobar"),
	(Version::new(0, 2, 0), "foobaz"),
];



//		Common

//		setup_core																
fn setup_core(releases_dir: &TempDir) -> Result<Core, ReleaseError> {
	Core::new(Config {
		appname:  s!("test"),
		releases: releases_dir.path().to_path_buf(),
		versions: VERSION_DATA.iter()
			.map(|(version, data)| (version.clone(), Sha256::digest(data).into()))
			.collect()
		,
	})
}

//		setup_files																
fn setup_files() -> TempDir {
	let releases_dir = tempdir().unwrap();
	for (version, data) in VERSION_DATA.iter() {
		let path     = releases_dir.path().join(&format!("test-{}", version));
		let mut file = File::create(&path).unwrap();
		write!(file, "{}", data).unwrap();
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
		let core = Core::new(Config {
			appname:  s!("test"),
			releases: tempdir().unwrap().path().to_path_buf(),
			versions: hash_map!{},
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
			Version::new(0, 0, 1): s!("baa5a0964d3320fbc0c6a922140453c8513ea24ab8fd0577034804a967248096"),
			Version::new(1, 1, 0): s!("c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2"),
			Version::new(0, 2, 0): s!("798f012674b5b8dcab4b00114bdf6738a69a4cdcf7ca0db1149260c9f81b73f7"),
		});
	}
	#[test]
	fn versions__empty() {
		let core = Core::new(Config {
			appname:  s!("test"),
			releases: tempdir().unwrap().path().to_path_buf(),
			versions: hash_map!{},
		}).unwrap();
		assert_eq!(core.versions(), hash_map!{});
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
		let unpacked = Axum::get_latest_version(Extension(core)).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				//	Axum automatically adds a content-type header.
				(s!("content-type"), s!("application/json")),
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
			Extension(core),
			Path(Version::new(0, 2, 0)),
		).await.into_response().unpack().unwrap();
		let crafted  = UnpackedResponse::new(
			StatusCode::OK,
			vec![
				//	Axum automatically adds a content-type header.
				(s!("content-type"), s!("application/json")),
			],
			json!({
				"version": s!("0.2.0"),
				"hash":    s!("798f012674b5b8dcab4b00114bdf6738a69a4cdcf7ca0db1149260c9f81b73f7"),
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
}


