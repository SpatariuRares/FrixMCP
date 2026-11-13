//! `X_AVM-DE_Homeauto:1`: gli `SmartHome` Device del router (docs/PROJECT.md §4.2.8).
//!
//! Unità di `x_homeauto.pdf` (AVM): potenza in 1/100 W, energia in Wh, temperature in
//! 0,1 °C. Da verificare sul router reale con le fixture registrate (#15).

use crate::error::FrixError;
use crate::services::{EntryList, EntryTruncation};
use crate::tr064::{SoapResponse, Tr064Client};

const SERVICE: &str = "X_AVM-DE_Homeauto:1";

/// Fault con cui il router risponde a un indice oltre l'ultimo `SmartHome` Device.
const SPECIFIED_ARRAY_INDEX_INVALID: u16 = 713;

/// Uno `SmartHome` Device, identificato dall'AIN. I testi sono dati non fidati.
#[derive(Debug, Clone, PartialEq)]
pub struct SmartHomeDevice {
    /// `NewAIN`: l'identità dello `SmartHome` Device.
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
    /// `NewPresent` in minuscolo: `connected`, `disconnected`, `registrered`, `unknown`.
    pub present: String,
    /// Presente solo se `NewSwitchIsEnabled` è `ENABLED`.
    pub switch: Option<Switch>,
    /// Presente solo se `NewMultimeterIsEnabled` è `ENABLED`.
    pub power_meter: Option<PowerMeter>,
    /// Presente solo se `NewTemperatureIsEnabled` è `ENABLED`.
    pub temperature: Option<Temperature>,
    /// Presente solo se `NewHkrIsEnabled` è `ENABLED`.
    pub thermostat: Option<Thermostat>,
}

/// Interruttore (presa). Stato `None` se il router lo dà come non valido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch {
    /// `NewSwitchState` in minuscolo (`on`, `off`, …).
    pub state: Option<String>,
    /// `NewSwitchMode` in minuscolo (`auto`, `manual`).
    pub mode: Option<String>,
    /// `NewSwitchLock`: comando manuale sull'apparecchio bloccato.
    pub locked: bool,
}

/// Misuratore di consumo. Valori `None` se il router li dà come non validi.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerMeter {
    /// `NewMultimeterPower`, convertita in W.
    pub power_w: Option<f64>,
    /// `NewMultimeterEnergy`, in Wh.
    pub energy_wh: Option<u32>,
}

/// Sensore di temperatura. Valori `None` se il router li dà come non validi.
#[derive(Debug, Clone, PartialEq)]
pub struct Temperature {
    /// `NewTemperatureCelsius`, convertita in °C.
    pub celsius: Option<f64>,
    /// `NewTemperatureOffset`, convertito in °C.
    pub offset_celsius: Option<f64>,
}

/// Termostato (HKR). Tutto `None` se il router lo dà come non valido.
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

/// Una temperatura impostata del termostato: valvola chiusa, aperta o regolata.
#[derive(Debug, Clone, PartialEq)]
pub struct Setpoint {
    /// `closed`, `open` o `temp` (solo con `temp` vale [`Setpoint::celsius`]).
    pub valve: String,
    /// La temperatura in °C, solo con valvola `temp`.
    pub celsius: Option<f64>,
}

/// `SmartHome` Device letti al massimo. FRITZ!OS ne gestisce qualche decina; senza
/// tetto un router che non risponde mai 713 costerebbe 65536 chiamate.
pub const MAX_DEVICES: usize = 256;

/// Gli `SmartHome` Device, per indice da 0 finché il router risponde 713, al massimo
/// [`MAX_DEVICES`]. Una voce non interpretabile si salta e si dichiara.
pub async fn list(client: &Tr064Client) -> Result<EntryList<SmartHomeDevice>, FrixError> {
    let mut devices = EntryList::default();
    for indice in (0..=u16::MAX).take(MAX_DEVICES) {
        match client
            .call(
                SERVICE,
                "GetGenericDeviceInfos",
                &[("NewIndex", &indice.to_string())],
            )
            .await
        {
            Ok(r) => devices.push(indice, device(&r))?,
            Err(FrixError::Soap {
                code: SPECIFIED_ARRAY_INDEX_INVALID,
                ..
            }) => return Ok(devices),
            Err(e) => return Err(e),
        }
    }
    // il tetto ha fermato la lettura: quante voci restino il router non lo dice
    devices.truncation = Some(EntryTruncation {
        limit: MAX_DEVICES,
        omitted: None,
    });
    Ok(devices)
}

fn device(r: &SoapResponse) -> Result<SmartHomeDevice, FrixError> {
    let testo = |nome: &str| r.text(nome).map(|t| t.trim().to_owned());
    let abilitato = |nome: &str| r.text(nome).is_ok_and(|t| t.trim() == "ENABLED");
    let valido = |nome: &str| r.text(nome).is_ok_and(|t| t.trim() == "VALID");
    let decimi = |nome: &str| r.parse::<i32>(nome).map(|v| f64::from(v) / 10.0);

    let switch = if abilitato("NewSwitchIsEnabled") {
        let valid = valido("NewSwitchIsValid");
        Some(Switch {
            state: valid.then(|| minuscolo(r, "NewSwitchState")).transpose()?,
            mode: valid.then(|| minuscolo(r, "NewSwitchMode")).transpose()?,
            locked: r.flag("NewSwitchLock")?,
        })
    } else {
        None
    };
    let power_meter = if abilitato("NewMultimeterIsEnabled") {
        let valid = valido("NewMultimeterIsValid");
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
    let temperature = if abilitato("NewTemperatureIsEnabled") {
        let valid = valido("NewTemperatureIsValid");
        Some(Temperature {
            celsius: valid.then(|| decimi("NewTemperatureCelsius")).transpose()?,
            offset_celsius: valid.then(|| decimi("NewTemperatureOffset")).transpose()?,
        })
    } else {
        None
    };
    let thermostat = if abilitato("NewHkrIsEnabled") {
        let valid = valido("NewHkrIsValid");
        let setpoint = |valvola: &str, gradi: &str| -> Result<Option<Setpoint>, FrixError> {
            if !valid {
                return Ok(None);
            }
            let valve = minuscolo(r, valvola)?;
            let celsius = if valve == "temp" {
                Some(decimi(gradi)?)
            } else {
                None
            };
            Ok(Some(Setpoint { valve, celsius }))
        };
        Some(Thermostat {
            current_celsius: valid.then(|| decimi("NewHkrIsTemperature")).transpose()?,
            target: setpoint("NewHkrSetVentilStatus", "NewHkrSetTemperature")?,
            reduced: setpoint("NewHkrReduceVentilStatus", "NewHkrReduceTemperature")?,
            comfort: setpoint("NewHkrComfortVentilStatus", "NewHkrComfortTemperature")?,
        })
    } else {
        None
    };

    Ok(SmartHomeDevice {
        ain: testo("NewAIN")?,
        device_id: r.parse("NewDeviceId")?,
        name: testo("NewDeviceName")?,
        product: testo("NewProductName")?,
        manufacturer: testo("NewManufacturer")?,
        firmware: testo("NewFirmwareVersion")?,
        function_bit_mask: r.parse("NewFunctionBitMask")?,
        present: minuscolo(r, "NewPresent")?,
        switch,
        power_meter,
        temperature,
        thermostat,
    })
}

fn minuscolo(r: &SoapResponse, nome: &str) -> Result<String, FrixError> {
    Ok(r.text(nome)?.trim().to_lowercase())
}
