//! The Patchify crate is an auto-update library, providing the ability for Rust
//! applications to automatically update themselves.
//! 



//		Global configuration

#![cfg_attr(feature = "reasons", feature(lint_reasons))]

//	Customisations of the standard linting configuration
#![cfg_attr(    feature = "reasons",  allow(clippy::multiple_crate_versions, reason = "Cannot resolve all these"))]
#![cfg_attr(not(feature = "reasons"), allow(clippy::multiple_crate_versions))]



//		Modules

pub mod server;
pub mod client;

mod responses;

#[cfg(test)]
#[path = "tests/common.rs"]
mod common;

#[cfg(test)]
#[path = "tests/mocks.rs"]
mod mocks;



//		Packages

#[cfg(test)]
mod integration_test_package_usage {
	use bytes as _;
	use test_binary as _;
	use tower_http as _;
	use tracing_subscriber as _;
	use wiremock as _;
}

#[cfg(test)]
mod examples_package_usage {
	use figment as _;
	use smart_default as _;
}


