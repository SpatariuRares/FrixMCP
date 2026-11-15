//! `FixtureTransport`: answers from files, without network. Used by tests (spec #1, "Testing Decisions").

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use super::{HttpRequest, HttpResponse, HttpTransport, Method, SendFuture};
use crate::error::FrixError;
use crate::tr064::SERVICE_URN_PREFIX;

/// Fixture transport: `GET` serves the description files, `POST` the SOAP responses
/// recorded per `SOAPACTION`. Every request received ends up in the [`RequestLog`].
///
/// On-disk layout of SOAP responses: `<Service>-<instance>/<Action>.xml`,
/// for example `DeviceInfo-1/GetInfo.xml` or `X_AVM-DE_HostFilter-1/GetWANAccessByIP.xml`.
/// A response valid only for certain input arguments (`Generic…` actions with an
/// index) is named `<Action>@<Argument>=<value>.xml`, for example
/// `GetGenericAssociatedDeviceInfo@NewAssociatedDeviceIndex=0.xml`.
#[derive(Debug, Default)]
pub struct FixtureTransport {
    files: HashMap<String, HttpResponse>,
    soap: HashMap<String, HttpResponse>,
    soap_by_args: Vec<ResponseByArgs>,
    log: RequestLog,
}

/// Response to `soap_action` when the request contains all the arguments `args`.
#[derive(Debug)]
struct ResponseByArgs {
    soap_action: String,
    args: Vec<(String, String)>,
    response: HttpResponse,
}

/// Shared log of the requests that reached a [`FixtureTransport`].
#[derive(Debug, Clone, Default)]
pub struct RequestLog(Arc<Mutex<Vec<HttpRequest>>>);

impl RequestLog {
    /// Copy of the requests received so far, in order.
    #[must_use]
    pub fn all(&self) -> Vec<HttpRequest> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Only the SOAP (`POST`) requests received so far.
    #[must_use]
    pub fn soap(&self) -> Vec<HttpRequest> {
        self.all()
            .into_iter()
            .filter(|r| r.method == Method::Post)
            .collect()
    }

    fn push(&self, request: HttpRequest) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request);
    }
}

impl FixtureTransport {
    /// Empty transport: every request fails with a transport error.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serves every file of `dir` at `GET /<file-name>` (`tr64desc.xml`, SCPD).
    pub fn with_descriptions_dir(mut self, dir: &Path) -> std::io::Result<Self> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let content = std::fs::read_to_string(&path)?;
                self.files
                    .insert(format!("/{name}"), HttpResponse::ok(content));
            }
        }
        Ok(self)
    }

    /// Serves `body` at `GET path`, replacing any file already there.
    #[must_use]
    pub fn with_file(self, path: &str, body: impl Into<String>) -> Self {
        self.with_get(path, HttpResponse::ok(body))
    }

    /// Answers `GET path` with `response`, for example an HTTP error.
    #[must_use]
    pub fn with_get(mut self, path: &str, response: HttpResponse) -> Self {
        self.files.insert(path.to_owned(), response);
        self
    }

    /// Loads the `200 OK` SOAP responses from `dir/<Service>-<instance>/<Action>.xml`.
    pub fn with_responses_dir(mut self, dir: &Path) -> std::io::Result<Self> {
        for service_dir in std::fs::read_dir(dir)? {
            let service_dir = service_dir?.path();
            let Some(name) = service_dir.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some((kind, instance)) = name.rsplit_once('-') else {
                continue;
            };
            let service = format!("{kind}:{instance}");
            for file in std::fs::read_dir(&service_dir)? {
                let file = file?.path();
                if file.extension().and_then(|e| e.to_str()) != Some("xml") {
                    continue;
                }
                let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                let body = HttpResponse::ok(std::fs::read_to_string(&file)?);
                self = match stem.split_once('@') {
                    Some((action, arg_list)) => {
                        let args: Vec<(&str, &str)> = arg_list
                            .split('&')
                            .filter_map(|pair| pair.split_once('='))
                            .collect();
                        self.with_soap_args(&service, action, &args, body)
                    }
                    None => self.with_soap(&service, stem, body),
                };
            }
        }
        Ok(self)
    }

    /// Response to `service#action`, for example `("DeviceInfo:1", "GetInfo")`;
    /// replaces a response already there (useful for faults and edge cases).
    #[must_use]
    pub fn with_soap(mut self, service: &str, action: &str, response: HttpResponse) -> Self {
        self.soap
            .insert(format!("{SERVICE_URN_PREFIX}{service}#{action}"), response);
        self
    }

    /// Response to `service#action` only when the request carries all the arguments
    /// `args` (for example `[("NewAssociatedDeviceIndex", "0")]`); takes precedence over
    /// [`Self::with_soap`] and replaces a response already there for the same arguments.
    /// Values are compared with the SOAP body as it was sent.
    #[must_use]
    pub fn with_soap_args(
        mut self,
        service: &str,
        action: &str,
        args: &[(&str, &str)],
        response: HttpResponse,
    ) -> Self {
        let soap_action = format!("{SERVICE_URN_PREFIX}{service}#{action}");
        let args: Vec<(String, String)> = args
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        self.soap_by_args
            .retain(|r| r.soap_action != soap_action || r.args != args);
        self.soap_by_args.push(ResponseByArgs {
            soap_action,
            args,
            response,
        });
        self
    }

    /// Log of the received requests, to keep before handing over the transport.
    #[must_use]
    pub fn requests(&self) -> RequestLog {
        self.log.clone()
    }

    fn respond(&self, request: &HttpRequest) -> Result<HttpResponse, FrixError> {
        let response = match (request.method, &request.soap_action) {
            (Method::Post, Some(soap_action)) => self
                .soap_by_args
                .iter()
                .find(|r| {
                    &r.soap_action == soap_action
                        && r.args.iter().all(|(name, value)| {
                            request.body.contains(&format!("<{name}>{value}</{name}>"))
                        })
                })
                .map(|r| r.response.clone())
                .or_else(|| self.soap.get(soap_action).cloned()),
            _ => self.files.get(&request.path).cloned(),
        };
        response.ok_or_else(|| {
            FrixError::Transport(format!(
                "no fixture for {:?} {} {:?}",
                request.method, request.path, request.soap_action
            ))
        })
    }
}

impl HttpTransport for FixtureTransport {
    fn send(&self, request: HttpRequest) -> SendFuture<'_> {
        let response = self.respond(&request);
        self.log.push(request);
        Box::pin(std::future::ready(response))
    }
}
