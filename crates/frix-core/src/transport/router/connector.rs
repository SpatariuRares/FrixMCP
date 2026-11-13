//! `Connector` di ureq che avvolge la connessione TCP in rustls con il `ClientConfig`
//! di [`super::tls`] (pin o CA). ureq è compilato senza TLS proprio: questo è l'unico
//! modo in cui una richiesta `https` può partire, e ureq rifiuta una richiesta `https`
//! su un trasporto che non è TLS.
//!
//! Ricalca `RustlsConnector` di ureq 3.5 (API `unversioned`).

use std::fmt;
use std::io::{Read as _, Write as _};
use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::{ClientConfig, ClientConnection, StreamOwned};
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout, Transport,
    TransportAdapter,
};

/// Avvolge in TLS le connessioni verso URL `https`; lascia passare le altre.
pub(super) struct RustlsConnector {
    config: Arc<ClientConfig>,
    server_name: ServerName<'static>,
}

impl RustlsConnector {
    pub(super) fn new(config: Arc<ClientConfig>, server_name: ServerName<'static>) -> Self {
        Self {
            config,
            server_name,
        }
    }
}

impl fmt::Debug for RustlsConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RustlsConnector")
            .field("server_name", &self.server_name)
            .finish_non_exhaustive()
    }
}

impl<In: Transport> Connector<In> for RustlsConnector {
    type Out = Either<In, TlsTransport>;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        let Some(transport) = chained else {
            return Ok(None);
        };
        if !details.needs_tls() || transport.is_tls() {
            return Ok(Some(Either::A(transport)));
        }
        let mut conn = ClientConnection::new(Arc::clone(&self.config), self.server_name.clone())
            .map_err(std::io::Error::other)?;
        let mut sock = TransportAdapter::new(transport.boxed(), Arc::clone(&details.current_time));
        sock.set_timeout(details.timeout);
        conn.complete_io(&mut sock)?;
        Ok(Some(Either::B(TlsTransport {
            buffers: LazyBuffers::new(
                details.config.input_buffer_size(),
                details.config.output_buffer_size(),
            ),
            stream: StreamOwned::new(conn, sock),
        })))
    }
}

/// Connessione TLS già stabilita.
pub(super) struct TlsTransport {
    buffers: LazyBuffers,
    stream: StreamOwned<ClientConnection, TransportAdapter>,
}

impl fmt::Debug for TlsTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsTransport").finish_non_exhaustive()
    }
}

impl Transport for TlsTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.stream.get_mut().set_timeout(timeout);
        let output = self.buffers.output().get(..amount).ok_or_else(|| {
            ureq::Error::Io(std::io::Error::other(
                "buffer di uscita più corto del previsto",
            ))
        })?;
        self.stream.write_all(output)?;
        Ok(())
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        self.stream.get_mut().set_timeout(timeout);
        let input = self.buffers.input_append_buf();
        let amount = self.stream.read(input)?;
        self.buffers.input_appended(amount);
        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        self.stream.get_mut().get_mut().is_open()
    }

    fn is_tls(&self) -> bool {
        true
    }
}
