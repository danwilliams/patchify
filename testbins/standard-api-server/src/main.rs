//		Modules

#[path = "../../../tests/common/mod.rs"]
mod common;



//		Packages

use crate::common::server::{initialize, create_server};
use tokio::signal;



//		Functions

//		main																	
#[tokio::main]
async fn main() {
	initialize();
	let (_address, _temp_dir) = create_server().await;
	signal::ctrl_c().await.unwrap();
}


