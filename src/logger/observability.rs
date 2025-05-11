#![allow(dead_code)]

use std::io::Write;

use reqwest::Client;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    Layer, Registry, filter,
    fmt::{self, MakeWriter, format},
    layer,
};

use crate::errors::Result;

pub struct Observe {
    client: Client,
    api_key: String,
    endpoint: String,
    // sender: Sender<String>,
}

type LayerAlias = filter::Filtered<
    fmt::Layer<
        layer::Layered<
            filter::Filtered<
                fmt::Layer<Registry, format::DefaultFields, format::Format<format::Compact>>,
                LevelFilter,
                Registry,
            >,
            Registry,
        >,
        format::DefaultFields,
        format::Format<format::Compact>,
        Observe,
    >,
    LevelFilter,
    layer::Layered<
        filter::Filtered<
            fmt::Layer<Registry, format::DefaultFields, format::Format<format::Compact>>,
            LevelFilter,
            Registry,
        >,
        Registry,
    >,
>;

impl Observe {
    pub fn new(api_key: String, endpoint: String) -> Self {
        let client = Client::new();
        Observe {
            client,
            api_key,
            endpoint,
            // sender,
        }
    }

    pub fn wrap_layer(self, level: LevelFilter) -> LayerAlias {
        fmt::layer()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_writer(self)
            .with_filter(level)
    }

    pub fn send_message<T: ToString>(&self, _msg: T) -> Result<()> {
        Ok(())
    }
}

impl Write for &Observe {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let message = String::from_utf8(buf.to_vec()).unwrap_or_default();
        println!("!!!message: {}", message);
        // if let Err(e) = self.sender.send(message) {
        //     eprintln!("Failed to send message: {}", e);
        // }
        // let mut reference of stdout
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // Implement the flush logic here
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Observe {
    type Writer = &'a Observe;

    fn make_writer(&'a self) -> Self::Writer {
        self
    }
}
