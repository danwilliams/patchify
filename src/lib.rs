//! The Patchify crate is an auto-update library, providing the ability for Rust
//! applications to automatically update themselves.
//! 



//		Global configuration

#![cfg_attr(feature = "reasons", feature(lint_reasons))]

//	Customisations of the standard linting configuration
#![cfg_attr(    feature = "reasons",  allow(clippy::multiple_crate_versions, reason = "Cannot resolve all these"))]
#![cfg_attr(not(feature = "reasons"), allow(clippy::multiple_crate_versions))]
#![cfg_attr(    feature = "reasons",  allow(clippy::items_after_test_module, reason = "Not needed with separated tests"))]
#![cfg_attr(not(feature = "reasons"), allow(clippy::items_after_test_module))]

//	Lints specifically disabled for unit tests
#![cfg_attr(test, allow(
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
	clippy::unwrap_in_result,
	clippy::unwrap_used,
))]



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


