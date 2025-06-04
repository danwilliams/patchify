//! An example of a simple CLI application, representing version 2.

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
	clippy::must_use_candidate,
	clippy::panic,
	clippy::print_stdout,
	clippy::tests_outside_test_module,
	clippy::too_many_lines,
	clippy::unwrap_in_result,
	clippy::unwrap_used,
	reason = "Not useful in examples"
)]



//		Packages

use core::time::Duration;
use figment::{
	Figment,
	providers::{Env, Format as _, Serialized, Toml},
};
use patchify::client::{Config as UpdaterConfig, Updater};
use rubedo::crypto::VerifyingKey;
use semver::Version;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::io::stdout;
use tokio::signal;
use tracing::{Level, info};
use tracing_subscriber::{
	EnvFilter,
	fmt::{format::FmtSpan, layer, writer::MakeWriterExt as _},
	layer::SubscriberExt as _,
	registry,
	util::SubscriberInitExt as _,
};



//		Structs

//		Config																	
/// The main configuration options for the application.
#[derive(Debug, Deserialize, Serialize, SmartDefault)]
pub struct Config {
	//		Public properties													
	/// The name of the application.
	#[default = "example"]
	pub appname:            String,
	
	/// The full location of the updater API server, including both FQDN and
	/// base path.
	#[default = "http://127.0.0.1:8000/api/"]
	pub updater_api_server: String,
	
	/// The public key of the updater API server. This is used to verify the
	/// server response signature.
	pub updater_api_key:    VerifyingKey,
	
	/// Whether to check for updates on startup.
	pub update_on_startup:  bool,
	
	/// The interval at which to check for updates, in seconds. If not provided,
	/// this will be disabled.
	pub update_interval:    Option<u64>,
}



//		Functions

//		main																	
#[tokio::main]
async fn main() {
	registry()
		.with(
			EnvFilter::new("info,reqwest=debug")
		)
		.with(
			layer()
				.with_writer(stdout.with_max_level(Level::INFO))
				.with_span_events(FmtSpan::NONE)
				.with_target(false)
		)
		.init()
	;
	let app_version    = Version::new(2, 0, 0);
	let config: Config = Figment::from(Serialized::defaults(Config::default()))
		.merge(Toml::file("cli-app.toml"))
		.merge(Env::raw())
		.extract()
		.expect("Error loading config")
	;
	let _updater = Updater::new(UpdaterConfig {
		version:          app_version.clone(),
		api:              config.updater_api_server.parse().expect("Invalid updater API server URL"),
		key:              config.updater_api_key,
		check_on_startup: config.update_on_startup,
		check_interval:   config.update_interval.map(Duration::from_secs),
	}).unwrap();
	info!("Application started");
	info!("{} v{app_version}", config.appname);
	signal::ctrl_c().await.unwrap();
	info!("Application stopped");
}


