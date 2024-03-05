//		Modules

#[allow(unused)]
#[path = "../../../tests/common/mod.rs"]
mod common;



//		Packages

use common::server::{initialize, create_basic_server, get_ping};
use axum::{Router, routing::get};
use ed25519_dalek::VerifyingKey;
use figment::{
	Figment,
	providers::Env,
};
use patchify::client::{Config as UpdaterConfig, Updater};
use semver::Version;
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use tokio::signal;



//		Structs

//		Config																	
#[derive(Deserialize)]
pub struct Config {
	pub api_port:   u16,
	pub public_key: String,
}



//		Functions

//		main																	
#[tokio::main]
async fn main() {
	initialize();
	let config: Config = Figment::new().merge(Env::raw()).extract().unwrap();
	let address        = create_basic_server(
		SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0)),
		Router::new()
			.route("/api/ping",    get(get_ping))
			.route("/api/version", get(get_version))
		,
	).await;
	let _updater = Updater::new(UpdaterConfig {
		version:          Version::new(1, 0, 0),
		api:              format!("http://127.0.0.1:{}/api/", config.api_port).parse().unwrap(),
		key:              VerifyingKey::from_bytes(&<[u8; 32]>::try_from(hex::decode(config.public_key).unwrap()).unwrap()).unwrap(),
		check_on_startup: true,
		check_interval:   None,
	}).unwrap();
	println!("Listening on: {address}");
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}

//		get_version																
async fn get_version() -> String {
	"1.0.0".to_owned()
}


