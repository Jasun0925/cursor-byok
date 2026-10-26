//! Detects changes to the inputs reqwest 0.12 / hyper-util 0.1 use for routing.
use std::hash::{Hash, Hasher};

use crate::store::ProxySettingsSecret;

// Keep REQUEST_METHOD: hyper-util suppresses HTTP_PROXY in CGI environments.
const ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
    "REQUEST_METHOD",
];

pub(super) fn read(settings: &ProxySettingsSecret) -> Option<u64> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    settings.mode.is_custom().hash(&mut hash);
    if settings.mode.is_custom() {
        settings.address.hash(&mut hash);
        settings.auth_enabled.hash(&mut hash);
        if settings.auth_enabled {
            settings.username.hash(&mut hash);
            settings.password.hash(&mut hash);
        }
    } else {
        for key in ENV_KEYS {
            std::env::var_os(key).hash(&mut hash);
        }
        #[cfg(target_os = "macos")]
        macos::hash_system_proxy(&mut hash)?;
        #[cfg(windows)]
        windows::hash_system_proxy(&mut hash)?;
    }
    // This value and its inputs are never logged (proxy URLs may contain credentials).
    Some(hash.finish())
}

#[cfg(target_os = "macos")]
mod macos {
    use std::hash::{Hash, Hasher};
    use system_configuration::{
        core_foundation::{
            base::CFType, dictionary::CFDictionary, number::CFNumber, string::CFString,
        },
        dynamic_store::SCDynamicStoreBuilder,
    };

    pub(super) fn hash_system_proxy(hash: &mut impl Hasher) -> Option<()> {
        let store = SCDynamicStoreBuilder::new("cursor-byok-proxy").build()?;
        let proxies = store.get_proxies()?;
        hash_settings(&proxies, hash);
        Some(())
    }

    fn hash_settings(proxies: &CFDictionary<CFString, CFType>, hash: &mut impl Hasher) {
        // These are the six SCDynamicStore keys read by hyper-util's matcher.
        // PAC/SOCKS/exclusions are not consumed by that backend on macOS.
        for scheme in ["HTTP", "HTTPS"] {
            let enabled = proxies
                .find(CFString::new(&format!("{scheme}Enable")))
                .and_then(|value| value.downcast::<CFNumber>())
                .and_then(|value| value.to_i32())
                == Some(1);
            enabled.hash(hash);
            if enabled {
                proxies
                    .find(CFString::new(&format!("{scheme}Proxy")))
                    .and_then(|value| value.downcast::<CFString>())
                    .map(|value| value.to_string())
                    .hash(hash);
                proxies
                    .find(CFString::new(&format!("{scheme}Port")))
                    .and_then(|value| value.downcast::<CFNumber>())
                    .and_then(|value| value.to_i32())
                    .hash(hash);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use system_configuration::core_foundation::base::TCFType;

        fn snapshot(enabled: i32, host: &str, port: i32, https: i32) -> u64 {
            let pairs = [
                (
                    CFString::new("HTTPEnable"),
                    CFNumber::from(enabled).as_CFType(),
                ),
                (CFString::new("HTTPProxy"), CFString::new(host).as_CFType()),
                (CFString::new("HTTPPort"), CFNumber::from(port).as_CFType()),
                (
                    CFString::new("HTTPSEnable"),
                    CFNumber::from(https).as_CFType(),
                ),
                (CFString::new("HTTPSProxy"), CFString::new(host).as_CFType()),
                (CFString::new("HTTPSPort"), CFNumber::from(port).as_CFType()),
            ];
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            hash_settings(&CFDictionary::from_CFType_pairs(&pairs), &mut hash);
            hash.finish()
        }

        #[test]
        fn macos_proxy_snapshot_tracks_enable_host_port_and_https() {
            let base = snapshot(1, "127.0.0.1", 7890, 1);
            assert_eq!(base, snapshot(1, "127.0.0.1", 7890, 1));
            for changed in [
                snapshot(0, "127.0.0.1", 7890, 1),
                snapshot(1, "127.0.0.2", 7890, 1),
                snapshot(1, "127.0.0.1", 7891, 1),
                snapshot(1, "127.0.0.1", 7890, 0),
            ] {
                assert_ne!(base, changed);
            }
            assert_eq!(snapshot(0, "old", 7890, 0), snapshot(0, "unused", 8888, 0));
        }

        #[test]
        fn macos_current_proxy_snapshot_can_be_read_without_modifying_it() {
            assert!(
                hash_system_proxy(&mut std::collections::hash_map::DefaultHasher::new()).is_some()
            );
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::hash::{Hash, Hasher};

    pub(super) fn hash_system_proxy(hash: &mut impl Hasher) -> Option<()> {
        let settings = windows_registry::CURRENT_USER
            .open("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
            .ok()?;
        let enabled = settings.get_u32("ProxyEnable").unwrap_or(0) != 0;
        enabled.hash(hash);
        if enabled {
            settings.get_string("ProxyServer").ok().hash(hash);
            settings.get_string("ProxyOverride").ok().hash(hash);
        }
        Some(())
    }
}
