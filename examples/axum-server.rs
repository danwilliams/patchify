//! An example of a simple Axum server that serves a patchify API.

#![allow(unused_crate_dependencies, reason = "Creates a lot of noise")]

//	Lints specifically disabled for examples
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
	reason = "Not useful in examples"
)]



//		Modules

#[expect(unused, reason = "Shared test code")]
#[path = "../tests/common/mod.rs"]
mod common;



//		Packages

use common::server::{initialize, create_patchify_api_server, patchify_api_routes};
use core::net::{IpAddr, SocketAddr};
use figment::{
	Figment,
	providers::{Env, Format, Serialized, Toml},
};
use rubedo::crypto::Sha256Hash;
use semver::Version;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{
	collections::HashMap,
	path::PathBuf,
};
use tokio::signal;



//		Structs

//		Config																	
/// The main configuration options for the application.
#[derive(Debug, Deserialize, Serialize, SmartDefault)]
pub struct Config {
	//		Public properties													
	/// The name of the application.
	#[default = "example"]
	pub appname:  String,
	
	/// The host to listen on.
	#[default(IpAddr::from([127, 0, 0, 1]))]
	pub host:     IpAddr,
	
	/// The port to listen on.
	#[default = 8000]
	pub port:     u16,
	
	/// The directory to store releases in.
	#[default = "releases"]
	pub releases: String,
	
	/// A list of version numbers and the SHA256 hashes of their release files.
	#[default(HashMap::new())]
	pub versions: HashMap<Version, Sha256Hash>,
}



//		Functions

//		main																	
#[tokio::main]
async fn main() {
	initialize();
	let config: Config = Figment::from(Serialized::defaults(Config::default()))
		.merge(Toml::file("axum-server.toml"))
		.merge(Env::raw())
		.extract()
		.expect("Error loading config")
	;
	let _address = create_patchify_api_server(
		&config.appname,
		SocketAddr::from((config.host, config.port)),
		patchify_api_routes(),
		PathBuf::from(config.releases),
		config.versions,
	);
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}


