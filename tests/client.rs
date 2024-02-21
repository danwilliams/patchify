#![allow(non_snake_case)]

//		Modules

#[allow(unused)]
mod common;



//		Packages

use crate::common::client::request;
use reqwest::StatusCode;
use std::{
	io::{BufReader, BufRead},
	net::SocketAddr,
	process::{Command, Stdio},
};
use test_binary::build_test_binary;



//		Tests

#[cfg(test)]
mod actions {
	use super::*;
	
	//		ping_server															
	#[tokio::test]
	async fn ping_server() {
		let testbin_path = build_test_binary("standard-api-server", "testbins").unwrap();
		let mut subproc  = Command::new(testbin_path).stdout(Stdio::piped()).spawn().unwrap();
		let reader       = BufReader::new(subproc.stdout.take().unwrap());
		let mut address  = String::new();
		for line in reader.lines() {
			let line     = line.unwrap();
			if line.contains("Listening on") {
				address  = line.split_whitespace().last().unwrap().to_owned();
				break;
			}
		}
		if address.is_empty() {
			panic!("Server address not found in stdout");
		}
		let address: SocketAddr  = address.parse().unwrap();
		let (status, _, _, body) = request(
			format!("http://{address}/api/ping"),
			None,
		).await;
		assert_eq!(status,        StatusCode::OK);
		assert_eq!(body.as_ref(), b"");
		subproc.kill().unwrap();
	}
}


