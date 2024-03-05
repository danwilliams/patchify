//		Modules

#[allow(unused)]
#[path = "../../../tests/common/mod.rs"]
mod common;



//		Packages

use common::server::{initialize, create_patchify_api_server, patchify_api_routes};
use figment::{
	Figment,
	providers::Env,
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
		"test".to_owned(),
		SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0)),
		patchify_api_routes(),
		PathBuf::from(config.releases),
		hash_map!{
			Version::new(1, 0, 0): <[u8; 32]>::try_from(hex::decode(config.version1).unwrap()).unwrap(),
			Version::new(2, 0, 0): <[u8; 32]>::try_from(hex::decode(config.version2).unwrap()).unwrap(),
		},
	).await;
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}


