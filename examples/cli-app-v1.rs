//		Packages

use ed25519_dalek::VerifyingKey;
use figment::{
	Figment,
	providers::{Env, Format, Serialized, Toml},
};
use hex;
use patchify::client::{Config as UpdaterConfig, Updater};
use semver::Version;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{
	io::stdout,
	time::Duration,
};
use tokio::signal;
use tracing::{Level, info};
use tracing_subscriber::{
	EnvFilter,
	fmt::{format::FmtSpan, layer, writer::MakeWriterExt},
	layer::SubscriberExt,
	registry,
	util::SubscriberInitExt,
};



//		Structs

//		Config																	
/// The main configuration options for the application.
#[derive(Deserialize, Serialize, SmartDefault)]
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
	#[default = "0000000000000000000000000000000000000000000000000000000000000000"]
	pub updater_api_key:    String,
	
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
	let app_version    = Version::new(1, 0, 0);
	let config: Config = Figment::from(Serialized::defaults(Config::default()))
		.merge(Toml::file("cli-app.toml"))
		.merge(Env::raw())
		.extract()
		.expect("Error loading config")
	;
	let _updater = Updater::new(UpdaterConfig {
		version:          app_version.clone(),
		api:              config.updater_api_server.parse().expect("Invalid updater API server URL"),
		key:              VerifyingKey::from_bytes(&<[u8; 32]>::try_from(
			hex::decode(config.updater_api_key).expect("Invalid public key")
		).expect("Invalid public key")).expect("Invalid public key"),
		check_on_startup: config.update_on_startup,
		check_interval:   config.update_interval.map(|secs| Duration::from_secs(secs)),
	});
	info!("Application started");
	info!("{} v{app_version}", config.appname);
	signal::ctrl_c().await.unwrap();
	info!("Application stopped");
}


