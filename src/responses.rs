//! This module provides functionality that is shared by client and server.

//		Modules

#[cfg(test)]
#[path = "tests/responses.rs"]
mod tests;



//		Packages

use rubedo::crypto::Sha256Hash;
use semver::Version;
use serde::{Deserialize, Serialize};



//		Structs

//		LatestVersionResponse													
/// The application version returned by the `latest` endpoint.
#[cfg_attr(    feature = "reasons",  allow(clippy::redundant_pub_crate, reason = "Internal use only"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::redundant_pub_crate))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct LatestVersionResponse {
	//		Crate-accessible properties											
	/// The latest version of the application.
	pub(crate) version: Version,
}

//		VersionHashResponse														
/// The application hash and version returned by the `hashes/:version` endpoint.
#[cfg_attr(    feature = "reasons",  allow(clippy::redundant_pub_crate, reason = "Internal use only"))]
#[cfg_attr(not(feature = "reasons"), allow(clippy::redundant_pub_crate))]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct VersionHashResponse {
	//		Crate-accessible properties											
	/// The requested version of the application.
	pub(crate) version: Version,
	
	/// The SHA256 hash of the application binary for this version.
	pub(crate) hash:    Sha256Hash,
}


