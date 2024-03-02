//		Modules

#[allow(unused)]
#[path = "../tests/common/server.rs"]
mod server;



//		Packages

use server::{initialize, create_server};
use figment::{
	Figment,
	providers::{Env, Format, Serialized, Toml},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use std::{
	collections::HashMap,
	net::IpAddr,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::signal;



//		Structs

//		Config																	
/// The main configuration options for the application.
#[derive(Deserialize, Serialize, SmartDefault)]
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
	pub versions: HashMap<Version, String>,
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
	let _address = create_server(
		config.appname,
		SocketAddr::from((config.host, config.port)),
		PathBuf::from(config.releases),
		config.versions.iter()
			.map(|(key, hash)| {(
				key.clone(),
				<[u8; 32]>::try_from(hex::decode(hash).expect("Invalid SHA256 hash")).expect("Invalid SHA256 hash"),
			)})
			.collect()
		,
	).await;
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}

