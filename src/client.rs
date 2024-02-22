//! This module provides client-side functionality to add to an application.

//		Modules

#[cfg(test)]
#[path = "tests/client.rs"]
mod tests;



//		Packages

use ed25519_dalek::VerifyingKey;
use reqwest::Url;
use semver::Version;
use tokio::{
	select,
	spawn,
	time::{Duration, interval},
};
use tracing::info;



//		Structs

//		Config																	
/// The configuration options for the client.
#[cfg_attr(    feature = "reasons",  allow(clippy::exhaustive_structs, reason = "Provided for configuration"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::exhaustive_structs))]
#[derive(Clone, Debug)]
pub struct Config {
	//		Public properties													
	/// The current version of the application. This is reported to the server
	/// when making requests such as checking for updates.
	pub version:          Version,
	
	/// The URL of the API. This is used to make requests to the server, such as
	/// checking for updates. It needs to be an FQDN (Fully-Qualified Domain
	/// Name), and should include the protocol (e.g. `https://`), plus any base
	/// path (e.g. `/api`). For example, `https://api.example.com/api/v2`.
	pub api:              Url,
	
	/// The public key for the server. This is used to verify the HTTP responses
	/// from the server, to ensure that they have not been tampered with. The
	/// format used is Ed25519, which is a modern and secure algorithm.
	pub key:              VerifyingKey,
	
	/// Whether to check for updates on startup.
	pub check_on_startup: bool,
	
	/// How often to check for updates. This is optional.
	pub check_interval:   Option<Duration>,
}

//		Updater																	
/// A service to check for updates at intervals, and upgrade the application.
/// 
/// This struct provides a service that will query the API server at defined
/// intervals, to check for updates. If an update is found, it will be
/// downloaded and installed, and the application will be restarted.
/// 
#[derive(Clone, Debug)]
pub struct Updater {
	//		Private properties													
	/// The configuration for the updater service.
	config: Config,
}

//󰭅		Updater																	
impl Updater {
	//		new																	
	/// Creates a new updater service instance.
	/// 
	/// This function creates a new updater service instance, with the specified
	/// configuration. As soon as the service is created, it will start checking
	/// for updates.
	/// 
	/// # Parameters
	/// 
	/// * `config` - The configuration for the updater service.
	/// 
	#[must_use]
	pub fn new(config: Config) -> Self {
		let updater = Self { config };
		if updater.config.check_on_startup {
			info!("Checking for updates");
		}
		if let Some(check_interval) = updater.config.check_interval {
			let mut timer = interval(check_interval);
			//	Interval-based loop
			drop(spawn(async move { loop { select!{
				_ = timer.tick() => {
					info!("Checking for updates");
				}
			}}}));
		}
		updater
	}
}


