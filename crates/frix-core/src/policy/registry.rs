//! Registro statico (servizio, azione) → Classification, compilato a mano dalla
//! docs/PROJECT.md §4.2 sui 35 SCPD del 7590 (FRITZ!OS 8.25). Il servizio è il tipo
//! senza istanza (`WLANConfiguration`, non `:1`): le tre istanze WLAN condividono lo SCPD.
//!
//! Regole per le azioni che la §4.2 non classifica esplicitamente (ogni caso è
//! commentato sulla riga):
//! - una lettura non citata, che non restituisce segreti, è `R`;
//! - una scrittura senza Level nella §4.2 ("fuori v1") è `X`: riclassificarla è una
//!   decisione esplicita di una spec successiva;
//! - fra due Level possibili si sceglie il più restrittivo.
//!
//! Un'azione nuova in un futuro FRITZ!OS non ha riga: la Policy la nega e il test
//! `ogni_azione_degli_scpd_del_7590_ha_una_classification` fallisce.

use super::{Classification, Level, Right};

/// Riga del registro: servizio, azione, Classification.
pub(super) type Entry = (&'static str, &'static str, Classification);

/// `read`: sempre attivo.
const R: Classification = Classification::Exposed(Level::Read);
/// `write`: `--allow-writes` e audit (spec successive).
const W: Classification = Classification::Exposed(Level::Write);
/// `dangerous`: `--allow-dangerous` e Confirmation (spec successive).
const D: Classification = Classification::Exposed(Level::Dangerous);
/// Forbidden: non eseguibile in nessun caso.
const X: Classification = Classification::Forbidden;

pub(super) const REGISTRY: &[Entry] = &[
    // §4.2.1 DeviceInfo
    ("DeviceInfo", "GetInfo", R),
    ("DeviceInfo", "GetDeviceLog", R),
    ("DeviceInfo", "X_AVM-DE_GetDeviceLogPath", R),
    ("DeviceInfo", "GetSecurityPort", R),
    ("DeviceInfo", "SetProvisioningCode", X),
    // §4.2.2 Hosts
    ("Hosts", "GetHostNumberOfEntries", R),
    ("Hosts", "GetGenericHostEntry", R),
    ("Hosts", "GetSpecificHostEntry", R),
    ("Hosts", "X_AVM-DE_GetSpecificHostEntryByIP", R),
    ("Hosts", "X_AVM-DE_GetHostListPath", R),
    ("Hosts", "X_AVM-DE_GetMeshListPath", R),
    ("Hosts", "X_AVM-DE_GetChangeCounter", R),
    ("Hosts", "X_AVM-DE_GetInfo", R),
    ("Hosts", "X_AVM-DE_GetAutoWakeOnLANByMACAddress", R),
    ("Hosts", "X_AVM-DE_GetFriendlyName", R),
    ("Hosts", "X_AVM-DE_WakeOnLANByMACAddress", W),
    ("Hosts", "X_AVM-DE_SetAutoWakeOnLANByMACAddress", W),
    ("Hosts", "X_AVM-DE_SetFriendlyNameByMAC", W),
    ("Hosts", "X_AVM-DE_SetFriendlyNameByIP", W),
    ("Hosts", "X_AVM-DE_SetHostNameByMACAddress", W),
    ("Hosts", "X_AVM-DE_SetDeviceClassUserByMAC", W),
    ("Hosts", "X_AVM-DE_SetDeviceClassUserByIP", W),
    ("Hosts", "X_AVM-DE_SetPrioritizationByIP", W),
    ("Hosts", "X_AVM-DE_SetFriendlyName", W),
    ("Hosts", "X_AVM-DE_HostsCheckUpdate", D),
    ("Hosts", "X_AVM-DE_HostDoUpdate", D),
    // §4.2.3 WLANConfiguration (istanze 1, 2, 3)
    ("WLANConfiguration", "GetInfo", R),
    ("WLANConfiguration", "GetSSID", R),
    ("WLANConfiguration", "GetBSSID", R),
    ("WLANConfiguration", "GetChannelInfo", R),
    ("WLANConfiguration", "GetStatistics", R),
    ("WLANConfiguration", "GetPacketStatistics", R),
    ("WLANConfiguration", "GetTotalAssociations", R),
    ("WLANConfiguration", "GetGenericAssociatedDeviceInfo", R),
    ("WLANConfiguration", "GetSpecificAssociatedDeviceInfo", R),
    (
        "WLANConfiguration",
        "X_AVM-DE_GetSpecificAssociatedDeviceInfoByIp",
        R,
    ),
    ("WLANConfiguration", "X_AVM-DE_GetWLANDeviceListPath", R),
    ("WLANConfiguration", "X_AVM-DE_GetWLANExtInfo", R),
    ("WLANConfiguration", "X_AVM-DE_GetWLANConnectionInfo", R),
    ("WLANConfiguration", "X_AVM-DE_GetWPSInfo", R),
    ("WLANConfiguration", "X_AVM-DE_GetNightControl", R),
    ("WLANConfiguration", "X_AVM-DE_GetIPTVOptimized", R),
    // §4.2.3 la elenca come R, ma lo SCPD del 7590 restituisce `NewKeyPassphrase`:
    // espone una chiave Wi-Fi come `GetSecurityKeys`, quindi stesso Level.
    ("WLANConfiguration", "X_AVM-DE_GetWLANHybridMode", D),
    ("WLANConfiguration", "GetBeaconType", R),
    ("WLANConfiguration", "GetBeaconAdvertisement", R),
    ("WLANConfiguration", "GetBasBeaconSecurityProperties", R),
    // espone la password Wi-Fi
    ("WLANConfiguration", "GetSecurityKeys", D),
    // §4.2.3: W sull'istanza ospiti, D sulle principali. La corrispondenza istanza →
    // Wi-Fi Network non è ancora verificata (§4.1), quindi D su tutte le istanze.
    ("WLANConfiguration", "SetEnable", D),
    ("WLANConfiguration", "X_AVM-DE_SetWLANGlobalEnable", D),
    // §4.2.3 "W/D" in ordine: SetWPSEnable W, SetWPSConfig D
    ("WLANConfiguration", "X_AVM-DE_SetWPSEnable", W),
    ("WLANConfiguration", "X_AVM-DE_SetWPSConfig", D),
    ("WLANConfiguration", "SetSSID", D),
    ("WLANConfiguration", "SetChannel", D),
    ("WLANConfiguration", "SetConfig", D),
    ("WLANConfiguration", "SetBeaconType", D),
    ("WLANConfiguration", "SetBeaconAdvertisement", D),
    ("WLANConfiguration", "SetBasBeaconSecurityProperties", D),
    // D su tutte le istanze, ospiti compresa (diritto A sulla rete ospiti)
    ("WLANConfiguration", "SetSecurityKeys", D),
    ("WLANConfiguration", "X_AVM-DE_SetStickSurfEnable", D),
    ("WLANConfiguration", "X_AVM-DE_SetIPTVOptimized", D),
    ("WLANConfiguration", "X_AVM-DE_SetWLANHybridMode", D),
    // WEP, "removed" da AVM
    ("WLANConfiguration", "GetDefaultWEPKeyIndex", X),
    ("WLANConfiguration", "SetDefaultWEPKeyIndex", X),
    // §4.2.4 WANCommonInterfaceConfig
    ("WANCommonInterfaceConfig", "GetCommonLinkProperties", R),
    ("WANCommonInterfaceConfig", "GetTotalBytesSent", R),
    ("WANCommonInterfaceConfig", "GetTotalBytesReceived", R),
    ("WANCommonInterfaceConfig", "GetTotalPacketsSent", R),
    ("WANCommonInterfaceConfig", "GetTotalPacketsReceived", R),
    ("WANCommonInterfaceConfig", "X_AVM-DE_GetOnlineMonitor", R),
    ("WANCommonInterfaceConfig", "X_AVM-DE_GetActiveProvider", R),
    ("WANCommonInterfaceConfig", "X_AVM-DE_SetWANAccessType", X),
    // §4.2.5 WANDSLInterfaceConfig
    ("WANDSLInterfaceConfig", "GetInfo", R),
    ("WANDSLInterfaceConfig", "GetStatisticsTotal", R),
    ("WANDSLInterfaceConfig", "X_AVM-DE_GetDSLInfo", R),
    ("WANDSLInterfaceConfig", "X_AVM-DE_GetDSLDiagnoseInfo", R),
    // §4.2.6 WANPPPConnection
    ("WANPPPConnection", "GetInfo", R),
    ("WANPPPConnection", "GetStatusInfo", R),
    ("WANPPPConnection", "GetExternalIPAddress", R),
    ("WANPPPConnection", "X_GetDNSServers", R),
    ("WANPPPConnection", "GetNATRSIPStatus", R),
    ("WANPPPConnection", "GetConnectionTypeInfo", R),
    ("WANPPPConnection", "GetLinkLayerMaxBitRates", R),
    ("WANPPPConnection", "GetPortMappingNumberOfEntries", R),
    ("WANPPPConnection", "GetGenericPortMappingEntry", R),
    ("WANPPPConnection", "GetSpecificPortMappingEntry", R),
    ("WANPPPConnection", "X_AVM-DE_GetAutoDisconnectTimeSpan", R),
    // non citata: lettura, lo UserName PPP è già in GetInfo (da non loggare)
    ("WANPPPConnection", "GetUserName", R),
    ("WANPPPConnection", "ForceTermination", D),
    ("WANPPPConnection", "RequestConnection", D),
    ("WANPPPConnection", "AddPortMapping", D),
    ("WANPPPConnection", "DeletePortMapping", D),
    ("WANPPPConnection", "SetConnectionType", D),
    ("WANPPPConnection", "SetConnectionTrigger", D),
    ("WANPPPConnection", "SetIdleDisconnectTime", D),
    ("WANPPPConnection", "SetRouteProtocolRx", D),
    ("WANPPPConnection", "X_AVM-DE_SetAutoDisconnectTimeSpan", D),
    ("WANPPPConnection", "SetUserName", X),
    ("WANPPPConnection", "SetPassword", X),
    // §4.2.6 WANIPConnection
    ("WANIPConnection", "GetInfo", R),
    ("WANIPConnection", "GetStatusInfo", R),
    ("WANIPConnection", "GetExternalIPAddress", R),
    ("WANIPConnection", "X_GetDNSServers", R),
    ("WANIPConnection", "GetNATRSIPStatus", R),
    ("WANIPConnection", "GetConnectionTypeInfo", R),
    ("WANIPConnection", "GetPortMappingNumberOfEntries", R),
    ("WANIPConnection", "GetGenericPortMappingEntry", R),
    ("WANIPConnection", "GetSpecificPortMappingEntry", R),
    ("WANIPConnection", "ForceTermination", D),
    ("WANIPConnection", "RequestConnection", D),
    ("WANIPConnection", "AddPortMapping", D),
    ("WANIPConnection", "DeletePortMapping", D),
    ("WANIPConnection", "SetConnectionType", D),
    ("WANIPConnection", "SetConnectionTrigger", D),
    ("WANIPConnection", "SetIdleDisconnectTime", D),
    ("WANIPConnection", "SetRouteProtocolRx", D),
    // §4.2.7 X_AVM-DE_HostFilter
    ("X_AVM-DE_HostFilter", "GetWANAccessByIP", R),
    ("X_AVM-DE_HostFilter", "GetHostEntryByIP", R),
    ("X_AVM-DE_HostFilter", "GetFilterProfiles", R),
    ("X_AVM-DE_HostFilter", "GetFilterProfileByID", R),
    ("X_AVM-DE_HostFilter", "GetTicketIDStatus", R),
    ("X_AVM-DE_HostFilter", "DisallowWANAccessByIP", W),
    ("X_AVM-DE_HostFilter", "AddHostEntryToFilterProfile", W),
    ("X_AVM-DE_HostFilter", "AddTicketTimeToHostEntryByIP", W),
    ("X_AVM-DE_HostFilter", "MarkTicket", W),
    ("X_AVM-DE_HostFilter", "DiscardAllTickets", W),
    // §4.2.8 X_AVM-DE_Homeauto
    ("X_AVM-DE_Homeauto", "GetInfo", R),
    ("X_AVM-DE_Homeauto", "GetGenericDeviceInfos", R),
    ("X_AVM-DE_Homeauto", "GetSpecificDeviceInfos", R),
    ("X_AVM-DE_Homeauto", "SetSwitch", W),
    ("X_AVM-DE_Homeauto", "SetDeviceName", W),
    // §4.2.9 X_AVM-DE_Dect
    ("X_AVM-DE_Dect", "GetNumberOfDectEntries", R),
    ("X_AVM-DE_Dect", "GetGenericDectEntry", R),
    ("X_AVM-DE_Dect", "GetSpecificDectEntry", R),
    ("X_AVM-DE_Dect", "GetDectListPath", R),
    ("X_AVM-DE_Dect", "DectDoUpdate", D),
    // §4.2.10 X_AVM-DE_Speedtest
    ("X_AVM-DE_Speedtest", "GetInfo", R),
    ("X_AVM-DE_Speedtest", "GetStatistics", R),
    ("X_AVM-DE_Speedtest", "SetConfig", W),
    ("X_AVM-DE_Speedtest", "ResetStatistics", W),
    // §4.2.11 X_AVM-DE_OnTel (le letture non citate per nome sono letture: R)
    ("X_AVM-DE_OnTel", "GetInfo", R),
    ("X_AVM-DE_OnTel", "GetInfoByIndex", R),
    ("X_AVM-DE_OnTel", "GetNumberOfEntries", R),
    ("X_AVM-DE_OnTel", "GetCallList", R),
    ("X_AVM-DE_OnTel", "GetPhonebookList", R),
    ("X_AVM-DE_OnTel", "GetPhonebook", R),
    ("X_AVM-DE_OnTel", "GetPhonebookEntry", R),
    ("X_AVM-DE_OnTel", "GetPhonebookEntryUID", R),
    ("X_AVM-DE_OnTel", "GetCallBarringEntry", R),
    ("X_AVM-DE_OnTel", "GetCallBarringEntryByNum", R),
    ("X_AVM-DE_OnTel", "GetCallBarringList", R),
    ("X_AVM-DE_OnTel", "GetDECTHandsetList", R),
    ("X_AVM-DE_OnTel", "GetDECTHandsetInfo", R),
    ("X_AVM-DE_OnTel", "GetNumberOfDeflections", R),
    ("X_AVM-DE_OnTel", "GetDeflection", R),
    ("X_AVM-DE_OnTel", "GetDeflections", R),
    ("X_AVM-DE_OnTel", "SetEnable", W),
    ("X_AVM-DE_OnTel", "SetConfig", W),
    ("X_AVM-DE_OnTel", "SetEnableByIndex", W),
    ("X_AVM-DE_OnTel", "SetConfigByIndex", W),
    ("X_AVM-DE_OnTel", "DeleteByIndex", W),
    ("X_AVM-DE_OnTel", "AddPhonebook", W),
    ("X_AVM-DE_OnTel", "DeletePhonebook", W),
    ("X_AVM-DE_OnTel", "SetPhonebookEntry", W),
    ("X_AVM-DE_OnTel", "SetPhonebookEntryUID", W),
    ("X_AVM-DE_OnTel", "DeletePhonebookEntry", W),
    ("X_AVM-DE_OnTel", "DeletePhonebookEntryUID", W),
    ("X_AVM-DE_OnTel", "SetCallBarringEntry", W),
    ("X_AVM-DE_OnTel", "DeleteCallBarringEntryUID", W),
    ("X_AVM-DE_OnTel", "SetDECTHandsetPhonebook", W),
    ("X_AVM-DE_OnTel", "SetDeflectionEnable", W),
    // §4.2.12 X_AVM-DE_TAM
    ("X_AVM-DE_TAM", "GetInfo", R),
    ("X_AVM-DE_TAM", "GetList", R),
    ("X_AVM-DE_TAM", "GetMessageList", R),
    ("X_AVM-DE_TAM", "SetEnable", W),
    ("X_AVM-DE_TAM", "MarkMessage", W),
    ("X_AVM-DE_TAM", "DeleteMessage", W),
    // §4.2.13 X_AVM-DE_Auth
    ("X_AVM-DE_Auth", "GetInfo", R),
    ("X_AVM-DE_Auth", "GetState", R),
    // senza Level nella §4.2 (avvia il secondo fattore, fase 4): X finché non si decide
    ("X_AVM-DE_Auth", "SetConfig", X),
    // §4.2.14 DeviceConfig
    // interno: SID per scaricare liste e log via HTTP, nessun tool lo espone
    ("DeviceConfig", "X_AVM-DE_CreateUrlSID", R),
    ("DeviceConfig", "ConfigurationStarted", W),
    ("DeviceConfig", "ConfigurationFinished", W),
    ("DeviceConfig", "X_AVM-DE_GetSupportDataInfo", R),
    ("DeviceConfig", "X_AVM-DE_GetSupportDataEnable", R),
    ("DeviceConfig", "X_AVM-DE_GetUserAgreement", R),
    ("DeviceConfig", "Reboot", D),
    ("DeviceConfig", "FactoryReset", X),
    ("DeviceConfig", "X_AVM-DE_GetConfigFile", X),
    ("DeviceConfig", "X_AVM-DE_SetConfigFile", X),
    ("DeviceConfig", "GetPersistentData", X),
    ("DeviceConfig", "SetPersistentData", X),
    ("DeviceConfig", "X_GenerateUUID", X),
    ("DeviceConfig", "X_AVM-DE_SendSupportData", X),
    ("DeviceConfig", "X_AVM-DE_SetSupportDataEnable", X),
    ("DeviceConfig", "X_AVM-DE_SetUserAgreement", X),
    // §4.2.15 X_AVM-DE_RemoteAccess
    ("X_AVM-DE_RemoteAccess", "GetInfo", R),
    ("X_AVM-DE_RemoteAccess", "GetDDNSInfo", R),
    ("X_AVM-DE_RemoteAccess", "GetDDNSProviders", R),
    ("X_AVM-DE_RemoteAccess", "SetConfig", X),
    ("X_AVM-DE_RemoteAccess", "SetEnable", X),
    ("X_AVM-DE_RemoteAccess", "SetDDNSConfig", X),
    ("X_AVM-DE_RemoteAccess", "SetLetsEncryptEnable", X),
    // §4.2.15 X_AVM-DE_MyFritz
    ("X_AVM-DE_MyFritz", "GetInfo", R),
    ("X_AVM-DE_MyFritz", "GetNumberOfServices", R),
    ("X_AVM-DE_MyFritz", "GetServiceByIndex", R),
    ("X_AVM-DE_MyFritz", "SetMyFRITZ", X),
    ("X_AVM-DE_MyFritz", "SetServiceByIndex", X),
    ("X_AVM-DE_MyFritz", "DeleteServiceByIndex", X),
    // §4.2.15 X_AVM-DE_USPController
    ("X_AVM-DE_USPController", "GetInfo", R),
    (
        "X_AVM-DE_USPController",
        "GetUSPControllerNumberOfEntries",
        R,
    ),
    ("X_AVM-DE_USPController", "GetUSPControllerByIndex", R),
    ("X_AVM-DE_USPController", "GetUSPMyFRITZEnable", R),
    ("X_AVM-DE_USPController", "AddUSPController", X),
    ("X_AVM-DE_USPController", "DeleteUSPControllerByIndex", X),
    ("X_AVM-DE_USPController", "SetUSPControllerEnableByIndex", X),
    ("X_AVM-DE_USPController", "SetUSPMyFRITZEnable", X),
    // §4.2.15 ManagementServer (TR-069)
    ("ManagementServer", "GetInfo", R),
    // non citata: lettura
    (
        "ManagementServer",
        "X_AVM-DE_GetTR069FirmwareDownloadEnabled",
        R,
    ),
    ("ManagementServer", "SetManagementServerURL", X),
    ("ManagementServer", "SetManagementServerUsername", X),
    ("ManagementServer", "SetManagementServerPassword", X),
    ("ManagementServer", "SetPeriodicInform", X),
    ("ManagementServer", "SetConnectionRequestAuthentication", X),
    ("ManagementServer", "SetUpgradeManagement", X),
    ("ManagementServer", "X_SetTR069Enable", X),
    (
        "ManagementServer",
        "X_AVM-DE_SetTR069FirmwareDownloadEnabled",
        X,
    ),
    // §4.2.16 LANHostConfigManagement
    ("LANHostConfigManagement", "GetInfo", R),
    ("LANHostConfigManagement", "GetAddressRange", R),
    ("LANHostConfigManagement", "GetDNSServers", R),
    ("LANHostConfigManagement", "GetIPRoutersList", R),
    ("LANHostConfigManagement", "GetSubnetMask", R),
    // non citata: lettura
    (
        "LANHostConfigManagement",
        "GetIPInterfaceNumberOfEntries",
        R,
    ),
    ("LANHostConfigManagement", "SetDHCPServerEnable", X),
    ("LANHostConfigManagement", "SetAddressRange", X),
    ("LANHostConfigManagement", "SetIPInterface", X),
    ("LANHostConfigManagement", "SetIPRouter", X),
    ("LANHostConfigManagement", "SetSubnetMask", X),
    // §4.2.16 LANEthernetInterfaceConfig
    ("LANEthernetInterfaceConfig", "GetInfo", R),
    ("LANEthernetInterfaceConfig", "GetStatistics", R),
    ("LANEthernetInterfaceConfig", "SetEnable", X),
    // §4.2.16 LANConfigSecurity
    ("LANConfigSecurity", "GetInfo", R),
    ("LANConfigSecurity", "X_AVM-DE_GetCurrentUser", R),
    ("LANConfigSecurity", "X_AVM-DE_GetAnonymousLogin", R),
    ("LANConfigSecurity", "X_AVM-DE_GetUserList", R),
    ("LANConfigSecurity", "SetConfigPassword", X),
    // §4.2.16 Layer3Forwarding
    ("Layer3Forwarding", "GetDefaultConnectionService", R),
    ("Layer3Forwarding", "GetForwardNumberOfEntries", R),
    ("Layer3Forwarding", "GetGenericForwardingEntry", R),
    // non citata: lettura
    ("Layer3Forwarding", "GetSpecificForwardingEntry", R),
    ("Layer3Forwarding", "AddForwardingEntry", X),
    ("Layer3Forwarding", "DeleteForwardingEntry", X),
    ("Layer3Forwarding", "SetDefaultConnectionService", X),
    ("Layer3Forwarding", "SetForwardingEntryEnable", X),
    // §4.2.16 Time
    ("Time", "GetInfo", R),
    ("Time", "SetNTPServers", D),
    // §4.2.16 UserInterface
    ("UserInterface", "GetInfo", R),
    ("UserInterface", "X_AVM-DE_GetInfo", R),
    ("UserInterface", "X_AVM-DE_GetInternationalConfig", R),
    ("UserInterface", "X_AVM-DE_CheckUpdate", W),
    ("UserInterface", "X_AVM-DE_DoUpdate", D),
    ("UserInterface", "X_AVM-DE_DoManualUpdate", X),
    ("UserInterface", "X_AVM-DE_SetConfig", D),
    ("UserInterface", "X_AVM-DE_SetInternationalConfig", X),
    // §4.2.16 "interno": prepara l'aggiornamento manuale (X) e frix-core non lo usa
    ("UserInterface", "X_AVM-DE_DoPrepareCGI", X),
    // §4.2.16 X_AVM-DE_Storage
    ("X_AVM-DE_Storage", "GetInfo", R),
    ("X_AVM-DE_Storage", "GetUserInfo", R),
    ("X_AVM-DE_Storage", "SetFTPServerWAN", X),
    ("X_AVM-DE_Storage", "RequestFTPServerWAN", D),
    ("X_AVM-DE_Storage", "SetFTPServer", D),
    ("X_AVM-DE_Storage", "SetSMBServer", D),
    ("X_AVM-DE_Storage", "SetUserConfig", D),
    // §4.2.16 X_AVM-DE_UPnP
    ("X_AVM-DE_UPnP", "GetInfo", R),
    ("X_AVM-DE_UPnP", "SetConfig", D),
    // §4.2.16 X_AVM-DE_WebDAVClient
    ("X_AVM-DE_WebDAVClient", "GetInfo", R),
    ("X_AVM-DE_WebDAVClient", "SetConfig", X),
    // §4.2.16 X_AVM-DE_Filelinks: scritture "fuori v1" senza Level (pubblicano file) → X
    ("X_AVM-DE_Filelinks", "GetNumberOfFilelinkEntries", R),
    ("X_AVM-DE_Filelinks", "GetGenericFilelinkEntry", R),
    ("X_AVM-DE_Filelinks", "GetSpecificFilelinkEntry", R),
    ("X_AVM-DE_Filelinks", "GetFilelinkListPath", R),
    ("X_AVM-DE_Filelinks", "NewFilelinkEntry", X),
    ("X_AVM-DE_Filelinks", "SetFilelinkEntry", X),
    ("X_AVM-DE_Filelinks", "DeleteFilelinkEntry", X),
    // §4.2.16 X_AVM-DE_Homeplug
    ("X_AVM-DE_Homeplug", "GetNumberOfDeviceEntries", R),
    ("X_AVM-DE_Homeplug", "GetGenericDeviceEntry", R),
    ("X_AVM-DE_Homeplug", "GetSpecificDeviceEntry", R),
    // senza Level nella §4.2: aggiornamento firmware come DectDoUpdate e HostDoUpdate
    ("X_AVM-DE_Homeplug", "DeviceDoUpdate", D),
    // §4.2.16 X_AVM-DE_WANMobileConnection: il 7590 non ha modem LTE, "ignorare (X)"
    ("X_AVM-DE_WANMobileConnection", "GetInfo", X),
    ("X_AVM-DE_WANMobileConnection", "GetInfoEx", X),
    ("X_AVM-DE_WANMobileConnection", "GetAccessTechnology", X),
    (
        "X_AVM-DE_WANMobileConnection",
        "GetPreferredAccessTechnology",
        X,
    ),
    (
        "X_AVM-DE_WANMobileConnection",
        "GetEnabledBandCapabilities",
        X,
    ),
    ("X_AVM-DE_WANMobileConnection", "GetBandCapabilities", X),
    ("X_AVM-DE_WANMobileConnection", "SetPIN", X),
    ("X_AVM-DE_WANMobileConnection", "SetPUK", X),
    ("X_AVM-DE_WANMobileConnection", "SetAccessTechnology", X),
    (
        "X_AVM-DE_WANMobileConnection",
        "SetPreferredAccessTechnology",
        X,
    ),
    (
        "X_AVM-DE_WANMobileConnection",
        "SetEnabledBandCapabilities",
        X,
    ),
    // §4.2.16 X_AVM-DE_AppSetup: GetConfig e GetAppRemoteInfo R, "il resto X"
    ("X_AVM-DE_AppSetup", "GetConfig", R),
    ("X_AVM-DE_AppSetup", "GetAppRemoteInfo", R),
    ("X_AVM-DE_AppSetup", "GetInfo", X),
    ("X_AVM-DE_AppSetup", "GetAppMessageFilter", X),
    ("X_AVM-DE_AppSetup", "GetBoxSenderId", X),
    ("X_AVM-DE_AppSetup", "RegisterApp", X),
    ("X_AVM-DE_AppSetup", "SetAppVPN", X),
    ("X_AVM-DE_AppSetup", "SetAppVPNwithPFS", X),
    ("X_AVM-DE_AppSetup", "SetAppMessageFilter", X),
    ("X_AVM-DE_AppSetup", "SetAppMessageReceiver", X),
    ("X_AVM-DE_AppSetup", "ResetEvent", X),
    // §4.2.16 X_VoIP: fuori v1. Letture R tranne X_AVM-DE_GetVoIPAccount (password SIP);
    // scritture senza Level (account, client, chiamate) → X
    ("X_VoIP", "GetInfo", R),
    ("X_VoIP", "GetInfoEx", R),
    ("X_VoIP", "GetMaxVoIPNumbers", R),
    ("X_VoIP", "GetExistingVoIPNumbers", R),
    ("X_VoIP", "GetVoIPCommonCountryCode", R),
    ("X_VoIP", "GetVoIPEnableCountryCode", R),
    ("X_VoIP", "GetVoIPCommonAreaCode", R),
    ("X_VoIP", "GetVoIPEnableAreaCode", R),
    ("X_VoIP", "X_AVM-DE_GetVoIPAccounts", R),
    ("X_VoIP", "X_AVM-DE_GetVoIPStatus", R),
    ("X_VoIP", "X_AVM-DE_GetNumberOfClients", R),
    ("X_VoIP", "X_AVM-DE_GetClient", R),
    ("X_VoIP", "X_AVM-DE_GetClient2", R),
    ("X_VoIP", "X_AVM-DE_GetClient3", R),
    ("X_VoIP", "X_AVM-DE_GetClientByClientId", R),
    ("X_VoIP", "X_AVM-DE_GetClients", R),
    ("X_VoIP", "X_AVM-DE_GetNumberOfNumbers", R),
    ("X_VoIP", "X_AVM-DE_GetNumbers", R),
    ("X_VoIP", "X_AVM-DE_DialGetConfig", R),
    ("X_VoIP", "X_AVM-DE_GetPhonePort", R),
    ("X_VoIP", "X_AVM-DE_GetVoIPCommonCountryCode", R),
    ("X_VoIP", "X_AVM-DE_GetVoIPCommonAreaCode", R),
    ("X_VoIP", "X_AVM-DE_GetAlarmClock", R),
    ("X_VoIP", "X_AVM-DE_GetNumberOfAlarmClocks", R),
    ("X_VoIP", "X_AVM-DE_GetVoIPAccount", X),
    ("X_VoIP", "SetConfig", X),
    ("X_VoIP", "SetVoIPCommonCountryCode", X),
    ("X_VoIP", "SetVoIPEnableCountryCode", X),
    ("X_VoIP", "SetVoIPCommonAreaCode", X),
    ("X_VoIP", "SetVoIPEnableAreaCode", X),
    ("X_VoIP", "X_AVM-DE_SetVoIPCommonCountryCode", X),
    ("X_VoIP", "X_AVM-DE_SetVoIPCommonAreaCode", X),
    ("X_VoIP", "X_AVM-DE_AddVoIPAccount", X),
    ("X_VoIP", "X_AVM-DE_DelVoIPAccount", X),
    ("X_VoIP", "X_AVM-DE_SetClient", X),
    ("X_VoIP", "X_AVM-DE_SetClient2", X),
    ("X_VoIP", "X_AVM-DE_SetClient3", X),
    ("X_VoIP", "X_AVM-DE_SetClient4", X),
    ("X_VoIP", "X_AVM-DE_DeleteClient", X),
    ("X_VoIP", "X_AVM-DE_DialSetConfig", X),
    ("X_VoIP", "X_AVM-DE_DialNumber", X),
    ("X_VoIP", "X_AVM-DE_DialHangup", X),
    ("X_VoIP", "X_AVM-DE_SetDelayedCallNotification", X),
    ("X_VoIP", "X_AVM-DE_SetAlarmClockEnable", X),
    // WANDSLLinkConfig: non è nella §4.2. Letture R; le scritture cambiano il link
    // WAN come X_AVM-DE_SetWANAccessType (X) → X
    ("WANDSLLinkConfig", "GetInfo", R),
    ("WANDSLLinkConfig", "GetAutoConfig", R),
    ("WANDSLLinkConfig", "GetDSLLinkInfo", R),
    ("WANDSLLinkConfig", "GetDestinationAddress", R),
    ("WANDSLLinkConfig", "GetATMEncapsulation", R),
    ("WANDSLLinkConfig", "GetStatistics", R),
    ("WANDSLLinkConfig", "SetEnable", X),
    ("WANDSLLinkConfig", "SetDSLLinkType", X),
    ("WANDSLLinkConfig", "SetDestinationAddress", X),
    ("WANDSLLinkConfig", "SetATMEncapsulation", X),
    // WANEthernetLinkConfig: non è nella §4.2, sola lettura
    ("WANEthernetLinkConfig", "GetEthernetLinkStatus", R),
];

/// Riga della tabella dei diritti: servizio, azione, diritti che bastano (uno qualunque).
pub(super) type RightsEntry = (&'static str, &'static str, &'static [Right]);

const NESSUNO: &[Right] = &[];
const C: &[Right] = &[Right::Configuration];
const A: &[Right] = &[Right::App];
const C_A: &[Right] = &[Right::Configuration, Right::App];
const C_H: &[Right] = &[Right::Configuration, Right::HomeAuto];
const C_A_P: &[Right] = &[Right::Configuration, Right::App, Right::Phone];
const C_A_P_N_H: &[Right] = &[
    Right::Configuration,
    Right::App,
    Right::Phone,
    Right::Nas,
    Right::HomeAuto,
];

/// Colonna "Diritto" della docs/PROJECT.md §4.2 (dal documento AVM *First Steps* v60,
/// cap. 6.1) per le azioni Read: è l'unica fonte del diritto che un fault 606 nomina.
/// Un'azione assente qui ha un diritto non documentato: nessuno lo inventa.
pub(super) const RIGHTS: &[RightsEntry] = &[
    // §4.2.1 DeviceInfo
    ("DeviceInfo", "GetInfo", C),
    ("DeviceInfo", "GetDeviceLog", C),
    ("DeviceInfo", "X_AVM-DE_GetDeviceLogPath", C),
    ("DeviceInfo", "GetSecurityPort", NESSUNO),
    // §4.2.2 Hosts
    ("Hosts", "GetHostNumberOfEntries", NESSUNO),
    ("Hosts", "GetGenericHostEntry", NESSUNO),
    ("Hosts", "GetSpecificHostEntry", NESSUNO),
    ("Hosts", "X_AVM-DE_GetSpecificHostEntryByIP", C_A_P),
    ("Hosts", "X_AVM-DE_GetHostListPath", C_A_P),
    ("Hosts", "X_AVM-DE_GetMeshListPath", C_A_P),
    ("Hosts", "X_AVM-DE_GetChangeCounter", NESSUNO),
    ("Hosts", "X_AVM-DE_GetInfo", NESSUNO),
    ("Hosts", "X_AVM-DE_GetAutoWakeOnLANByMACAddress", NESSUNO),
    ("Hosts", "X_AVM-DE_GetFriendlyName", NESSUNO),
    // §4.2.3 WLANConfiguration
    ("WLANConfiguration", "GetInfo", C_A_P),
    ("WLANConfiguration", "GetSSID", A),
    ("WLANConfiguration", "GetBSSID", C),
    ("WLANConfiguration", "GetChannelInfo", C),
    ("WLANConfiguration", "GetStatistics", C),
    ("WLANConfiguration", "GetPacketStatistics", C),
    ("WLANConfiguration", "GetTotalAssociations", C_A_P),
    ("WLANConfiguration", "GetGenericAssociatedDeviceInfo", C_A_P),
    (
        "WLANConfiguration",
        "GetSpecificAssociatedDeviceInfo",
        C_A_P,
    ),
    (
        "WLANConfiguration",
        "X_AVM-DE_GetSpecificAssociatedDeviceInfoByIp",
        C_A_P,
    ),
    ("WLANConfiguration", "X_AVM-DE_GetWLANDeviceListPath", C_A_P),
    ("WLANConfiguration", "X_AVM-DE_GetWLANExtInfo", C_A),
    (
        "WLANConfiguration",
        "X_AVM-DE_GetWLANConnectionInfo",
        NESSUNO,
    ),
    ("WLANConfiguration", "X_AVM-DE_GetWPSInfo", C_A),
    ("WLANConfiguration", "X_AVM-DE_GetNightControl", NESSUNO),
    ("WLANConfiguration", "X_AVM-DE_GetIPTVOptimized", C),
    // §4.2.4 WANCommonInterfaceConfig
    (
        "WANCommonInterfaceConfig",
        "GetCommonLinkProperties",
        C_A_P_N_H,
    ),
    ("WANCommonInterfaceConfig", "GetTotalBytesSent", C),
    ("WANCommonInterfaceConfig", "GetTotalBytesReceived", C),
    ("WANCommonInterfaceConfig", "GetTotalPacketsSent", C),
    ("WANCommonInterfaceConfig", "GetTotalPacketsReceived", C),
    ("WANCommonInterfaceConfig", "X_AVM-DE_GetOnlineMonitor", C),
    (
        "WANCommonInterfaceConfig",
        "X_AVM-DE_GetActiveProvider",
        C_A_P,
    ),
    // §4.2.5 WANDSLInterfaceConfig
    ("WANDSLInterfaceConfig", "GetInfo", C),
    ("WANDSLInterfaceConfig", "GetStatisticsTotal", C),
    ("WANDSLInterfaceConfig", "X_AVM-DE_GetDSLInfo", NESSUNO),
    (
        "WANDSLInterfaceConfig",
        "X_AVM-DE_GetDSLDiagnoseInfo",
        NESSUNO,
    ),
    // §4.2.6 WANPPPConnection / WANIPConnection
    ("WANPPPConnection", "GetInfo", C),
    ("WANPPPConnection", "GetStatusInfo", C),
    ("WANPPPConnection", "GetExternalIPAddress", C),
    ("WANPPPConnection", "X_GetDNSServers", C),
    ("WANPPPConnection", "GetNATRSIPStatus", C),
    ("WANPPPConnection", "GetConnectionTypeInfo", C),
    ("WANPPPConnection", "GetLinkLayerMaxBitRates", C),
    ("WANPPPConnection", "GetPortMappingNumberOfEntries", C),
    ("WANPPPConnection", "GetGenericPortMappingEntry", C),
    ("WANPPPConnection", "GetSpecificPortMappingEntry", C),
    ("WANPPPConnection", "X_AVM-DE_GetAutoDisconnectTimeSpan", C),
    ("WANIPConnection", "GetInfo", C),
    ("WANIPConnection", "GetStatusInfo", C),
    ("WANIPConnection", "GetExternalIPAddress", C),
    ("WANIPConnection", "X_GetDNSServers", C),
    ("WANIPConnection", "GetNATRSIPStatus", C),
    ("WANIPConnection", "GetConnectionTypeInfo", C),
    ("WANIPConnection", "GetPortMappingNumberOfEntries", C),
    ("WANIPConnection", "GetGenericPortMappingEntry", C),
    ("WANIPConnection", "GetSpecificPortMappingEntry", C),
    // §4.2.7 X_AVM-DE_HostFilter
    ("X_AVM-DE_HostFilter", "GetWANAccessByIP", C_A),
    ("X_AVM-DE_HostFilter", "GetHostEntryByIP", C_A),
    ("X_AVM-DE_HostFilter", "GetFilterProfiles", C_A),
    ("X_AVM-DE_HostFilter", "GetFilterProfileByID", C_A),
    ("X_AVM-DE_HostFilter", "GetTicketIDStatus", C_A),
    // §4.2.8 X_AVM-DE_Homeauto
    ("X_AVM-DE_Homeauto", "GetInfo", C_H),
    ("X_AVM-DE_Homeauto", "GetGenericDeviceInfos", C_H),
    ("X_AVM-DE_Homeauto", "GetSpecificDeviceInfos", C_H),
    // §4.2.14 DeviceConfig
    ("DeviceConfig", "X_AVM-DE_CreateUrlSID", C_A_P_N_H),
    // §4.3: LANHostConfigManagement:GetInfo richiede C.
    ("LANHostConfigManagement", "GetInfo", C),
];
