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
	let (_address, _temp_dir) = create_test_server().await;
	signal::ctrl_c().await.unwrap();
	println!("Shutting down");
}


