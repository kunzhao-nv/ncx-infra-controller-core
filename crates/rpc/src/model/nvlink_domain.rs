/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

//! Domain wire conversions with explicit role and membership validation.

use model::controller_outcome::PersistentStateHandlerOutcome;
use model::nvlink_domain::{
    NvLinkDomain, NvLinkDomainComponentType, NvLinkDomainConfig, NvLinkDomainMember,
    NvLinkDomainState,
};

use crate::errors::RpcDataConversionError;
use crate::forge;
use crate::model::expected_rack_group::validate_rack_group_id;

impl TryFrom<forge::NvLinkDomainMember> for NvLinkDomainMember {
    type Error = RpcDataConversionError;

    fn try_from(member: forge::NvLinkDomainMember) -> Result<Self, Self::Error> {
        let component_type = match forge::NvLinkDomainComponentType::try_from(member.component_type)
        {
            Ok(forge::NvLinkDomainComponentType::Compute) => NvLinkDomainComponentType::Compute,
            Ok(forge::NvLinkDomainComponentType::Switch) => NvLinkDomainComponentType::Switch,
            _ => {
                return Err(RpcDataConversionError::InvalidArgument(
                    "component_type must be Compute or Switch".into(),
                ));
            }
        };
        let rack_id = member
            .rack_id
            .ok_or(RpcDataConversionError::MissingArgument("rack_id"))?;
        let rack_id = rack_id.as_str().parse().map_err(|_| {
            RpcDataConversionError::InvalidArgument("rack_id must not be empty".into())
        })?;
        Ok(Self {
            component_type,
            rack_id,
            tray_index: member
                .tray_index
                .filter(|index| *index >= 0)
                .ok_or_else(|| {
                    RpcDataConversionError::InvalidArgument(
                        "tray_index must be present and non-negative".into(),
                    )
                })?,
        })
    }
}

impl From<NvLinkDomainMember> for forge::NvLinkDomainMember {
    fn from(member: NvLinkDomainMember) -> Self {
        Self {
            component_type: match member.component_type {
                NvLinkDomainComponentType::Compute => forge::NvLinkDomainComponentType::Compute,
                NvLinkDomainComponentType::Switch => forge::NvLinkDomainComponentType::Switch,
            } as i32,
            rack_id: Some(member.rack_id),
            tray_index: Some(member.tray_index),
        }
    }
}

impl TryFrom<forge::NvLinkDomainConfig> for NvLinkDomainConfig {
    type Error = RpcDataConversionError;

    fn try_from(config: forge::NvLinkDomainConfig) -> Result<Self, Self::Error> {
        let rack_group_id = config
            .rack_group_id
            .ok_or(RpcDataConversionError::MissingArgument("rack_group_id"))?;
        validate_rack_group_id(&rack_group_id)?;
        if config.domain_profile_id.trim().is_empty() {
            return Err(RpcDataConversionError::InvalidArgument(
                "domain_profile_id must not be empty".into(),
            ));
        }
        Ok(Self {
            rack_group_id,
            domain_profile_id: config.domain_profile_id,
            members: config
                .members
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl From<NvLinkDomainConfig> for forge::NvLinkDomainConfig {
    fn from(config: NvLinkDomainConfig) -> Self {
        Self {
            rack_group_id: Some(config.rack_group_id),
            domain_profile_id: config.domain_profile_id,
            members: config.members.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<NvLinkDomain> for forge::NvLinkDomain {
    fn from(domain: NvLinkDomain) -> Self {
        let state = match domain.controller_state.value {
            NvLinkDomainState::Discovering => forge::NvLinkDomainLifecycleState::Discovering,
            NvLinkDomainState::Configuring { .. } => forge::NvLinkDomainLifecycleState::Configuring,
            NvLinkDomainState::Ready => forge::NvLinkDomainLifecycleState::Ready,
            NvLinkDomainState::Maintenance { .. } => forge::NvLinkDomainLifecycleState::Maintenance,
            NvLinkDomainState::Deleting => forge::NvLinkDomainLifecycleState::Deleting,
        };
        let state_reason = match domain.controller_state_outcome {
            Some(PersistentStateHandlerOutcome::Wait { reason, .. }) => Some(reason),
            Some(PersistentStateHandlerOutcome::Error { err, .. }) => Some(err),
            _ => None,
        };
        Self {
            id: Some(domain.id.into()),
            metadata: Some(domain.metadata.into()),
            config: Some(domain.config.into()),
            status: Some(forge::NvLinkDomainStatus {
                state: state as i32,
                nmxc_domain_id: domain.nmxc_domain_id.map(Into::into),
                deletion_requested: domain.deleted.is_some(),
                state_reason,
                state_version: domain.controller_state.version.to_string(),
            }),
            version: domain.version.to_string(),
            managed: domain.managed,
        }
    }
}

#[cfg(test)]
mod tests {
    use carbide_test_support::{Check, check_values};

    use super::*;

    #[test]
    fn config_wire_validation_and_round_trip() {
        let request = forge::NvLinkDomainConfig {
            rack_group_id: Some("group".parse().unwrap()),
            domain_profile_id: "MiNi".into(),
            members: vec![forge::NvLinkDomainMember {
                component_type: forge::NvLinkDomainComponentType::Compute as i32,
                rack_id: Some("rack".parse().unwrap()),
                tray_index: Some(0),
            }],
        };
        type Mutation = fn(&mut forge::NvLinkDomainConfig);
        let cases: &[(&str, Mutation)] = &[
            ("missing group", |r| r.rack_group_id = None),
            ("blank profile", |r| r.domain_profile_id = " ".into()),
            ("unspecified role", |r| r.members[0].component_type = 0),
            ("unknown role", |r| r.members[0].component_type = 100),
            ("missing rack", |r| r.members[0].rack_id = None),
            ("empty rack", |r| {
                r.members[0].rack_id = Some(Default::default())
            }),
            ("missing index", |r| r.members[0].tray_index = None),
            ("negative index", |r| r.members[0].tray_index = Some(-1)),
        ];
        check_values(
            cases.iter().map(|(name, mutate)| Check {
                scenario: *name,
                input: {
                    let mut invalid = request.clone();
                    mutate(&mut invalid);
                    invalid
                },
                expect: true,
            }),
            |invalid| NvLinkDomainConfig::try_from(invalid).is_err(),
        );
        let model = NvLinkDomainConfig::try_from(request.clone()).unwrap();
        assert_eq!(forge::NvLinkDomainConfig::from(model), request);
    }
}
