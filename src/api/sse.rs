use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::State,
    response::{sse::Event, Sse},
};
use serde::Serialize;
use tokio_stream::{Stream, StreamExt};

use super::{AppState, StatusResponse, ServerConfig};

#[derive(Serialize)]
struct StreamUpdate {
    status: StatusResponse,
    config: ServerConfig,
}

pub async fn stream_updates(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut last_status_version: u64 = 0;
    
    let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(1)))
        .filter_map(move |_| {
            let info = state.service.build_full_status();
            let config = state.service.get_config();
            
            if info.version == last_status_version {
                return None;
            }
            last_status_version = info.version;

            let update = StreamUpdate {
                status: StatusResponse::from(info),
                config: ServerConfig::from(config),
            };

            Some(Ok(Event::default().json_data(update).unwrap()))
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(10))
    )
}