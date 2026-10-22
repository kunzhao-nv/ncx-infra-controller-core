/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

//! Fabric profiles independent of physical Rack inventory and power shelves.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ConfigValidationError;
use crate::nvlink_domain::{NvLinkDomainComponentType, NvLinkDomainConfig};
use crate::rack_type::{
    RackCapabilityCompute, RackCapabilitySwitch, RackFirmwareObjectConfig, RackHardwareClass,
    RackHardwareTopology, RackHardwareType, RackProductFamily,
};

/// Site-configured fabric identity and required tray capabilities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainProfile {
    /// RMS fabric topology; required and independent of physical Rack topology.
    pub topology: RackHardwareTopology,
    /// Product family used for component descriptors.
    pub product_family: RackProductFamily,
    /// Hardware selector for firmware artifacts; absent uses backend defaults.
    #[serde(default)]
    pub hardware_type: Option<RackHardwareType>,
    /// Hardware class used by component backends.
    #[serde(default)]
    pub hardware_class: Option<RackHardwareClass>,
    /// Optional firmware source for Domain maintenance.
    #[serde(default)]
    pub firmware_object: Option<RackFirmwareObjectConfig>,
    /// Shared descriptor attributes; component attributes override matching keys.
    #[serde(default)]
    pub attributes: HashMap<String, String>,
    /// Compute descriptors and exact required tray count.
    pub compute: RackCapabilityCompute,
    /// Switch descriptors and exact required tray count.
    pub switch: RackCapabilitySwitch,
}

impl DomainProfile {
    /// Reject a declaration whose role counts do not match the selected profile.
    /// Physical connectivity validation remains a fabric-controller responsibility.
    pub fn validate_members(
        &self,
        config: &NvLinkDomainConfig,
    ) -> Result<(), ConfigValidationError> {
        for (role, required) in [
            (NvLinkDomainComponentType::Compute, self.compute.count),
            (NvLinkDomainComponentType::Switch, self.switch.count),
        ] {
            let actual = config
                .members
                .iter()
                .filter(|member| member.component_type == role)
                .count();
            if required == 0 || actual != required as usize {
                return Err(ConfigValidationError::InvalidValue(format!(
                    "domain profile requires {required} {role:?} trays, received {actual}"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use carbide_test_support::{Check, check_values};

    use super::*;
    use crate::nvlink_domain::NvLinkDomainMember;

    #[test]
    fn profile_counts_are_role_specific_and_nonzero() {
        let profile: DomainProfile = serde_json::from_value(serde_json::json!({
            "topology": "gb200_nvl72r1_c2g4_topology",
            "product_family": "gb200",
            "compute": { "count": 1 },
            "switch": { "count": 1 }
        }))
        .unwrap();
        let config = NvLinkDomainConfig {
            rack_group_id: "group".parse().unwrap(),
            domain_profile_id: "mini".into(),
            members: [
                NvLinkDomainComponentType::Compute,
                NvLinkDomainComponentType::Switch,
            ]
            .into_iter()
            .map(|component_type| NvLinkDomainMember {
                component_type,
                rack_id: "rack".parse().unwrap(),
                tray_index: 0,
            })
            .collect(),
        };
        check_values(
            [
                ("matching roles", 1, 1, true),
                ("missing compute", 2, 1, false),
                ("missing switch", 1, 2, false),
                ("zero required compute", 0, 1, false),
            ]
            .map(|(scenario, compute, switch, expect)| Check {
                scenario,
                input: (compute, switch),
                expect,
            }),
            |(compute, switch)| {
                let mut profile = profile.clone();
                profile.compute.count = compute;
                profile.switch.count = switch;
                profile.validate_members(&config).is_ok()
            },
        );
    }
}
