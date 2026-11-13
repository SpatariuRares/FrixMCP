//! `list_smarthome_devices` (docs/PROJECT.md §4.2.8, §8.1).

use frix_core::FrixError;
use frix_core::services::EntryTruncation;
use frix_core::services::homeauto::{self, Setpoint, SmartHomeDevice};
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};
use serde::Serialize;

use crate::response::{self, VoceSaltata};
use crate::server::FrixServer;

#[derive(Debug, Serialize)]
struct Devices {
    devices: Vec<DeviceOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<VoceSaltata>,
}

#[derive(Debug, Serialize)]
struct DeviceOut {
    ain: String,
    device_id: u16,
    name: String,
    product: String,
    manufacturer: String,
    firmware: String,
    function_bit_mask: u16,
    present: String,
    switch: Option<SwitchOut>,
    power_meter: Option<PowerMeterOut>,
    temperature: Option<TemperatureOut>,
    thermostat: Option<ThermostatOut>,
}

#[derive(Debug, Serialize)]
struct SwitchOut {
    state: Option<String>,
    mode: Option<String>,
    locked: bool,
}

#[derive(Debug, Serialize)]
struct PowerMeterOut {
    power_w: Option<f64>,
    energy_wh: Option<u32>,
}

#[derive(Debug, Serialize)]
struct TemperatureOut {
    celsius: Option<f64>,
    offset_celsius: Option<f64>,
}

#[derive(Debug, Serialize)]
struct ThermostatOut {
    current_celsius: Option<f64>,
    target: Option<SetpointOut>,
    reduced: Option<SetpointOut>,
    comfort: Option<SetpointOut>,
}

#[derive(Debug, Serialize)]
struct SetpointOut {
    valve: String,
    celsius: Option<f64>,
}

impl From<Setpoint> for SetpointOut {
    fn from(s: Setpoint) -> Self {
        Self {
            valve: s.valve,
            celsius: s.celsius,
        }
    }
}

impl From<SmartHomeDevice> for DeviceOut {
    fn from(d: SmartHomeDevice) -> Self {
        Self {
            ain: d.ain,
            device_id: d.device_id,
            name: d.name,
            product: d.product,
            manufacturer: d.manufacturer,
            firmware: d.firmware,
            function_bit_mask: d.function_bit_mask,
            present: d.present,
            switch: d.switch.map(|s| SwitchOut {
                state: s.state,
                mode: s.mode,
                locked: s.locked,
            }),
            power_meter: d.power_meter.map(|m| PowerMeterOut {
                power_w: m.power_w,
                energy_wh: m.energy_wh,
            }),
            temperature: d.temperature.map(|t| TemperatureOut {
                celsius: t.celsius,
                offset_celsius: t.offset_celsius,
            }),
            thermostat: d.thermostat.map(|t| ThermostatOut {
                current_celsius: t.current_celsius,
                target: t.target.map(Into::into),
                reduced: t.reduced.map(Into::into),
                comfort: t.comfort.map(Into::into),
            }),
        }
    }
}

#[tool_router(router = smarthome_router, vis = "pub(crate)")]
impl FrixServer {
    #[tool(
        name = "list_smarthome_devices",
        description = concat!(
            "SmartHome Device del FRITZ!Box, identificati per AIN: nome, presenza, interruttore delle prese, potenza in W ed energia in Wh, temperatura in °C, termostato (temperatura attuale, impostata, ridotta, comfort). ",
            dati_della_rete!()
        ),
        annotations(
            title = "Elenco degli SmartHome Device",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_smarthome_devices(&self) -> Result<CallToolResult, ErrorData> {
        Ok(response::from_result_con_tetto(
            self.smarthome_devices().await,
        ))
    }
}

impl FrixServer {
    async fn smarthome_devices(&self) -> Result<(Devices, Option<EntryTruncation>), FrixError> {
        let lista = homeauto::list(&self.client).await?;
        let dati = Devices {
            devices: lista.entries.into_iter().map(Into::into).collect(),
            skipped: lista.skipped.into_iter().map(Into::into).collect(),
        };
        Ok((dati, lista.truncation))
    }
}
