use std::{
    io::{Cursor, Write},
    time::Duration,
};

use anyhow::Context;
use gethostname::gethostname;
use rand::{Rng, SeedableRng};
use services::content_directory::ContentDirectoryBrowseProvider;
use ssdp::SsdpRunner;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

mod constants;
mod http_server;
pub mod services;
mod ssdp;
pub mod state;
mod subscriptions;
mod templates;

pub struct UpnpServerOptions {
    pub friendly_name: String,
    pub http_listen_port: u16,
    pub http_prefix: String,
    pub browse_provider: Box<dyn ContentDirectoryBrowseProvider>,
    pub cancellation_token: CancellationToken,
}

pub struct UpnpServer {
    axum_router: Option<axum::Router>,
    ssdp_runner: SsdpRunner,
}

fn create_usn(opts: &UpnpServerOptions) -> anyhow::Result<String> {
    let mut buf = [0u8; 32];
    let mut cursor = Cursor::new(&mut buf[..]);

    let _ = cursor.write_all(gethostname().as_encoded_bytes());
    let _ = write!(
        &mut cursor,
        "{}{}{}",
        opts.friendly_name, opts.http_listen_port, opts.http_prefix
    );

    let mut uuid = [0u8; 16];
    rand::rngs::SmallRng::from_seed(buf).fill(&mut uuid);
    let uuid = uuid::Builder::from_slice(&uuid)
        .context("error generating UUID")?
        .into_uuid();
    Ok(format!("uuid:{uuid}"))
}

impl UpnpServer {
    pub async fn new(opts: UpnpServerOptions) -> anyhow::Result<Self> {
        let usn = create_usn(&opts).context("error generating USN")?;

        let description_http_location = {
            let port = opts.http_listen_port;
            let http_prefix = &opts.http_prefix;
            let surl = format!("http://0.0.0.0:{port}{http_prefix}/description.xml");
            url::Url::parse(&surl)
                .context(surl)
                .context("error parsing url")?
        };

        info!(
            location = %description_http_location,
            "starting UPnP/SSDP announcer for MediaServer"
        );
        let ssdp_runner = crate::ssdp::SsdpRunner::new(ssdp::SsdpRunnerOptions {
            usn: usn.clone(),
            description_http_location,
            server_string: "Linux/3.4 UPnP/1.0 rqbit/1".to_owned(),
            notify_interval: Duration::from_secs(60),
            shutdown: opts.cancellation_token.clone(),
        })
        .await
        .context("error initializing SsdpRunner")?;

        let router = crate::http_server::make_router(
            opts.friendly_name,
            opts.http_prefix,
            usn,
            opts.browse_provider,
            opts.cancellation_token,
        )?;

        Ok(Self {
            axum_router: Some(router),
            ssdp_runner,
        })
    }

    pub fn take_router(&mut self) -> anyhow::Result<axum::Router> {
        self.axum_router
            .take()
            .context("programming error: router already taken")
    }

    pub async fn run_ssdp_forever(&self) -> anyhow::Result<()> {
        debug!("starting SSDP");
        self.ssdp_runner
            .run_forever()
            .await
            .context("error running SSDP loop")
    }
}
