//! `X_AVM-DE_Homeauto:1`: the router's `SmartHome` Devices (docs/PROJECT.md §4.2.8).
//!
//! Units from `x_homeauto.pdf` (AVM): power in 1/100 W, energy in Wh, temperatures in
//! 0.1 °C. To be verified on the real router with recorded fixtures (#15).

use crate::error::FrixError;
use crate::services::{EntryList, EntryTruncation};
use crate::tr064::{SoapResponse, Tr064Client};

const SERVICE: &str = "X_AVM-DE_Homeauto:1";

/// Fault the router returns for an index past the last `SmartHome` Device.
const SPECIFIED_ARRAY_INDEX_INVALID: u16 = 713;

/// A `SmartHome` Device, identified by its AIN. Texts are untrusted data.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartHomeDevice {
    /// `NewAIN`: the identity of the `SmartHome` Device.
    pub ain: String,
    /// `NewDeviceId`.
    pub device_id: u16,
    /// `NewDeviceName`.
    pub name: String,
    /// `NewProductName`.
    pub product: String,
    /// `NewManufacturer`.
    pub manufacturer: String,
    /// `NewFirmwareVersion`.
    pub firmware: String,
    /// `NewFunctionBitMask`.
    pub function_bit_mask: u16,
    /// `NewPresent` in lowercase: `connected`, `disconnected`, `registrered`, `unknown`.
    pub present: String,
    /// Present only if `NewSwitchIsEnabled` is `ENABLED`.
    pub switch: Option<Switch>,
    /// Present only if `NewMultimeterIsEnabled` is `ENABLED`.
    pub power_meter: Option<PowerMeter>,
    /// Present only if `NewTemperatureIsEnabled` is `ENABLED`.
    pub temperature: Option<Temperature>,
    /// Present only if `NewHkrIsEnabled` is `ENABLED`.
    pub thermostat: Option<Thermostat>,
}

/// Switch (plug). State `None` if the router reports it as invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch {
    /// `NewSwitchState` in lowercase (`on`, `off`, …).
    pub state: Option<String>,
    /// `NewSwitchMode` in lowercase (`auto`, `manual`).
    pub mode: Option<String>,
    /// `NewSwitchLock`: manual control on the device is locked.
    pub locked: bool,
}

/// Power meter. Values `None` if the router reports them as invalid.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerMeter {
    /// `NewMultimeterPower`, converted to W.
    pub power_w: Option<f64>,
    /// `NewMultimeterEnergy`, in Wh.
    pub energy_wh: Option<u32>,
}

/// Temperature sensor. Values `None` if the router reports them as invalid.
#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    /// `NewTemperatureCelsius`, converted to °C.
    pub celsius: Option<f64>,
    /// `NewTemperatureOffset`, converted to °C.
    pub offset_celsius: Option<f64>,
}

/// Thermostat (HKR). All `None` if the router reports it as invalid.
#[derive(Debug, Clone, PartialEq)]
pub struct Thermostat {
    /// `NewHkrIsTemperature`, in °C.
    pub current_celsius: Option<f64>,
    /// `NewHkrSetVentilStatus` + `NewHkrSetTemperature`.
    pub target: Option<Setpoint>,
    /// `NewHkrReduceVentilStatus` + `NewHkrReduceTemperature`.
    pub reduced: Option<Setpoint>,
    /// `NewHkrComfortVentilStatus` + `NewHkrComfortTemperature`.
    pub comfort: Option<Setpoint>,
}

/// A thermostat setpoint: valve closed, open or regulated.
#[derive(Debug, Clone, PartialEq)]
pub struct Setpoint {
    /// `closed`, `open` or `temp` ([`Setpoint::celsius`] is set only with `temp`).
    pub valve: String,
    /// The temperature in °C, only with valve `temp`.
    pub celsius: Option<f64>,
}

/// Maximum `SmartHome` Devices read. FRITZ!OS handles a few dozen; without a
/// cap a router that never answers 713 would cost 65536 calls.
pub const MAX_DEVICES: usize = 256;

/// The `SmartHome` Devices, by index from 0 until the router answers 713, at most
/// [`MAX_DEVICES`]. An unparsable entry is skipped and reported.
pub async fn list(client: &Tr064Client) -> Result<EntryList<SmartHomeDevice>, FrixError> {
    let mut devices = EntryList::default();
    for index in (0..=u16::MAX).take(MAX_DEVICES) {
        match client
            .call(
                SERVICE,
                "GetGenericDeviceInfos",
                &[("NewIndex", &index.to_string())],
            )
            .await
        {
            Ok(r) => devices.push(index, device(&r))?,
            Err(FrixError::Soap {
                code: SPECIFIED_ARRAY_INDEX_INVALID,
                ..
            }) => return Ok(devices),
            Err(e) => return Err(e),
        }
    }
    // the cap stopped the read: the router does not say how many entries remain
    devices.truncation = Some(EntryTruncation {
        limit: MAX_DEVICES,
        omitted: None,
    });
    Ok(devices)
}

fn device(r: &SoapResponse) -> Result<SmartHomeDevice, FrixError> {
    let text = |name: &str| r.text(name).map(|t| t.trim().to_owned());
    let is_enabled = |name: &str| r.text(name).is_ok_and(|t| t.trim() == "ENABLED");
    let is_valid = |name: &str| r.text(name).is_ok_and(|t| t.trim() == "VALID");
    let tenths = |name: &str| r.parse::<i32>(name).map(|v| f64::from(v) / 10.0);

    let switch = if is_enabled("NewSwitchIsEnabled") {
        let valid = is_valid("NewSwitchIsValid");
        Some(Switch {
            state: valid.then(|| lowercase(r, "NewSwitchState")).transpose()?,
            mode: valid.then(|| lowercase(r, "NewSwitchMode")).transpose()?,
            locked: r.flag("NewSwitchLock")?,
        })
    } else {
        None
    };
    let power_meter = if is_enabled("NewMultimeterIsEnabled") {
        let valid = is_valid("NewMultimeterIsValid");
        Some(PowerMeter {
            power_w: valid
                .then(|| {
                    r.parse::<u32>("NewMultimeterPower")
                        .map(|v| f64::from(v) / 100.0)
                })
                .transpose()?,
            energy_wh: valid
                .then(|| r.parse::<u32>("NewMultimeterEnergy"))
                .transpose()?,
        })
    } else {
        None
    };
    let temperature = if is_enabled("NewTemperatureIsEnabled") {
        let valid = is_valid("NewTemperatureIsValid");
        Some(Temperature {
            celsius: valid.then(|| tenths("NewTemperatureCelsius")).transpose()?,
            offset_celsius: valid.then(|| tenths("NewTemperatureOffset")).transpose()?,
        })
    } else {
        None
    };
    let thermostat = if is_enabled("NewHkrIsEnabled") {
        let valid = is_valid("NewHkrIsValid");
        let setpoint = |valve_arg: &str, temp_arg: &str| -> Result<Option<Setpoint>, FrixError> {
            if !valid {
                return Ok(None);
            }
            let valve = lowercase(r, valve_arg)?;
            let celsius = if valve == "temp" {
                Some(tenths(temp_arg)?)
            } else {
                None
            };
            Ok(Some(Setpoint { valve, celsius }))
        };
        Some(Thermostat {
            current_celsius: valid.then(|| tenths("NewHkrIsTemperature")).transpose()?,
            target: setpoint("NewHkrSetVentilStatus", "NewHkrSetTemperature")?,
            reduced: setpoint("NewHkrReduceVentilStatus", "NewHkrReduceTemperature")?,
            comfort: setpoint("NewHkrComfortVentilStatus", "NewHkrComfortTemperature")?,
        })
    } else {
        None
    };

    Ok(SmartHomeDevice {
        ain: text("NewAIN")?,
        device_id: r.parse("NewDeviceId")?,
        name: text("NewDeviceName")?,
        product: text("NewProductName")?,
        manufacturer: text("NewManufacturer")?,
        firmware: text("NewFirmwareVersion")?,
        function_bit_mask: r.parse("NewFunctionBitMask")?,
        present: lowercase(r, "NewPresent")?,
        switch,
        power_meter,
        temperature,
        thermostat,
    })
}

fn lowercase(r: &SoapResponse, name: &str) -> Result<String, FrixError> {
    Ok(r.text(name)?.trim().to_lowercase())
}
