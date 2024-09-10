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

use common::server::{initialize, create_basic_server, get_ping};
use axum::{Router, routing::get};
use rubedo::crypto::VerifyingKey;
use serde::Deserialize;
use std::net::{IpAddr, SocketAddr};
use tokio::signal;



//		Structs

//		Config																	
#[derive(Deserialize)]
pub struct Config {
	pub api_port:   u16,
	pub public_key: VerifyingKey,
}



//		Functions

//		main																	
#[tokio::main]
async fn main() {
	initialize();
	let address = create_basic_server(
		SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0)),
		Router::new()
			.route("/api/ping",    get(get_ping))
			.route("/api/version", get(get_version))
		,
	).await;
	println!("Listening on: {address}");
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}

//		get_version																
async fn get_version() -> String {
	"2.0.0".to_owned()
}


