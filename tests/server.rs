#![allow(non_snake_case)]

//		Packages

use axum::{
	Router,
	Server,
	http::{HeaderMap, StatusCode},
	routing::get,
};
use bytes::Bytes;
use reqwest::Client;
use std::{
	io::stdout,
	net::{IpAddr, SocketAddr},
	sync::{Arc, Once},
	time::Duration,
};
use tokio::spawn;
use tower_http::{
	LatencyUnit,
	classify::ServerErrorsFailureClass,
	trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, Span, debug, error, info};
use tracing_subscriber::{
	EnvFilter,
	fmt::{format::FmtSpan, layer, writer::MakeWriterExt},
	layer::SubscriberExt,
	registry,
	util::SubscriberInitExt,
};



//		Statics

static INIT: Once = Once::new();



//		Structs

//		AppState																
struct AppState {
}



//		Functions

//		initialize																
fn initialize() {
	INIT.call_once(|| {
		registry()
			.with(
				EnvFilter::new("server=debug,reqwest=debug,tower_http=debug")
			)
			.with(
				layer()
					.with_writer(stdout.with_max_level(Level::INFO))
					.with_span_events(FmtSpan::NONE)
					.with_target(false)
			)
			.init()
		;
	});
}

//		create_server															
async fn create_server() -> SocketAddr {
	let app = Router::new()
		.route("/api/ping", get(get_ping))
		.with_state(Arc::new(AppState {
		}))
		.layer(TraceLayer::new_for_http()
			.on_request(
				DefaultOnRequest::new()
					.level(Level::INFO)
			)
			.on_response(
				DefaultOnResponse::new()
					.level(Level::INFO)
					.latency_unit(LatencyUnit::Micros)
			)
			.on_body_chunk(|chunk: &Bytes, _latency: Duration, _span: &Span| {
				debug!("Sending {} bytes", chunk.len())
			})
			.on_eos(|_trailers: Option<&HeaderMap>, stream_duration: Duration, _span: &Span| {
				debug!("Stream closed after {:?}", stream_duration)
			})
			.on_failure(|_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {
				error!("Something went wrong")
			})
		)
	;
	let server  = Server::bind(&SocketAddr::from((IpAddr::from([127, 0, 0, 1]), 0))).serve(app.into_make_service());
	let address = server.local_addr();
	info!("Listening on {address}");
	spawn(server);
	address
}

//		get_ping																
async fn get_ping() {}



//		Tests

#[cfg(test)]
mod endpoints {
	use super::*;
	
	//		get_ping															
	#[tokio::test]
	async fn get_ping() {
		initialize();
		let address  = spawn(async { create_server().await }).await.unwrap();
		let response = Client::new().get(format!("http://{address}/api/ping")).send().await.unwrap();
		assert_eq!(response.status(),              StatusCode::OK);
		assert_eq!(response.text().await.unwrap(), "");
	}
}


