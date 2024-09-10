#![allow(unused_crate_dependencies, reason = "Creates a lot of noise")]

//	Lints specifically disabled for tests
#![allow(
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
	clippy::tests_outside_test_module,
	clippy::unwrap_in_result,
	clippy::unwrap_used,
	reason = "Not useful in tests"
)]



//		Modules

#[allow(unused)]
#[path = "../../../tests/common/mod.rs"]
mod common;



//		Packages

use common::server::{initialize, create_test_server};
use tokio::signal;



//		Functions

//		main																	
#[tokio::main]
async fn main() {
	initialize();
	let (_address, _temp_dir) = create_test_server();
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}


