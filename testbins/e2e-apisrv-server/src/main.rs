#![allow(unused_crate_dependencies, reason = "Creates a lot of noise")]

//	Lints specifically disabled for tests
#![allow(
	non_snake_case,
	unreachable_pub,
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
	clippy::must_use_candidate,
	clippy::panic,
	clippy::print_stdout,
	clippy::tests_outside_test_module,
	clippy::unwrap_in_result,
	clippy::unwrap_used,
	reason = "Not useful in tests"
)]



//		Modules

#[expect(unused)]
#[path = "../../../tests/common/mod.rs"]
mod common;



//		Packages

use common::server::{initialize, create_patchify_api_server, patchify_api_routes};
use figment::{
	Figment,
	providers::Env,
};
use rubedo::{
	crypto::Sha256Hash,
	std::ByteSized,
};
use semver::Version;
use serde::Deserialize;
use std::{
	net::{IpAddr, SocketAddr},
	path::PathBuf,
};
use tokio::signal;
use velcro::hash_map;



//		Structs

//		Config																	
#[derive(Deserialize)]
pub struct Config {
	pub releases: String,
	pub version1: String,
	pub version2: String,
}



//		Functions

//		main																	
#[tokio::main]
async fn main() {
	initialize();
	let config: Config = Figment::new().merge(Env::raw()).extract().unwrap();
	let _address       = create_patchify_api_server(
		"test",
		SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0)),
		patchify_api_routes(),
		PathBuf::from(config.releases),
		hash_map!{
			Version::new(1, 0, 0): Sha256Hash::from_hex(&config.version1).unwrap(),
			Version::new(2, 0, 0): Sha256Hash::from_hex(&config.version2).unwrap(),
		},
	).await;
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}


