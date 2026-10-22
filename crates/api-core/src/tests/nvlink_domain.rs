/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

use rpc::forge;
use rpc::forge::forge_server::Forge;
use tonic::{Code, Request};

use crate::tests::common::api_fixtures::{TestEnvOverrides, create_test_env_with_overrides};

#[crate::sqlx_test()]
async fn domain_api_persists_config_and_retains_deleting_members(pool: sqlx::PgPool) {
    let mut config = crate::test_support::default_config::get();
    config.domain_profiles.insert(
        "mini".into(),
        serde_json::from_value(serde_json::json!({
            "topology": "gb200_nvl72r1_c2g4_topology",
            "product_family": "gb200",
            "compute": { "count": 1 },
            "switch": { "count": 1 }
        }))
        .unwrap(),
    );
    let env =
        create_test_env_with_overrides(pool.clone(), TestEnvOverrides::with_config(config)).await;
    sqlx::raw_sql(r#"INSERT INTO expected_rack_groups (rack_group_id, topology, racks)
        VALUES ('group', 'topology', '[{"rack_id":"rack","members":[]} ]');
        INSERT INTO expected_machines (serial_number, bmc_mac_address, bmc_username, bmc_password, rack_id)
        VALUES ('compute', '00:11:22:33:44:55', 'test', 'test', 'rack');
        INSERT INTO expected_switches (serial_number, bmc_mac_address, bmc_username, bmc_password, rack_id)
        VALUES ('switch', '00:11:22:33:44:66', 'test', 'test', 'rack');"#)
        .execute(&pool).await.unwrap();
    let members = [
        (
            forge::NvLinkDomainComponentType::Compute,
            "00:11:22:33:44:55",
        ),
        (
            forge::NvLinkDomainComponentType::Switch,
            "00:11:22:33:44:66",
        ),
    ]
    .into_iter()
    .map(|(component_type, mac)| forge::NvLinkDomainMember {
        component_type: component_type as i32,
        bmc_mac_address: mac.into(),
    })
    .collect();
    let request = forge::CreateNvLinkDomainRequest {
        config: Some(forge::NvLinkDomainConfig {
            rack_group_id: Some("group".parse().unwrap()),
            domain_profile_id: "mini".into(),
            members,
        }),
        metadata: Some(forge::Metadata {
            name: "mini-a".into(),
            ..Default::default()
        }),
    };
    let mut invalid = request.clone();
    invalid.config.as_mut().unwrap().domain_profile_id = "MINI".into();
    assert_eq!(
        env.api
            .create_nv_link_domain(Request::new(invalid))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let mut invalid = request.clone();
    invalid.config.as_mut().unwrap().members.pop();
    assert_eq!(
        env.api
            .create_nv_link_domain(Request::new(invalid))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let domain = env
        .api
        .create_nv_link_domain(Request::new(request.clone()))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(domain.config, request.config);
    assert_eq!(domain.metadata, request.metadata);
    assert_eq!(
        domain.status.as_ref().unwrap().state(),
        forge::NvLinkDomainLifecycleState::Discovering
    );
    assert!(!domain.managed);
    let lookup = forge::NvLinkDomainRequest {
        id: domain.id.clone(),
    };
    assert_eq!(
        env.api
            .get_nv_link_domain(Request::new(lookup.clone()))
            .await
            .unwrap()
            .into_inner(),
        domain
    );
    assert_eq!(
        env.api
            .find_nv_link_domain_ids(Request::new(forge::NvLinkDomainSearchFilter::default()))
            .await
            .unwrap()
            .into_inner()
            .ids,
        vec![domain.id.clone().unwrap()]
    );
    assert_eq!(
        env.api
            .find_nv_link_domains_by_ids(Request::new(forge::NvLinkDomainsByIdsRequest {
                ids: vec![domain.id.clone().unwrap()]
            }))
            .await
            .unwrap()
            .into_inner()
            .domains,
        vec![domain.clone()]
    );
    for ids in [
        vec![],
        vec![domain.id.clone().unwrap(); env.api.runtime_config.max_find_by_ids as usize + 1],
    ] {
        assert_eq!(
            env.api
                .find_nv_link_domains_by_ids(Request::new(forge::NvLinkDomainsByIdsRequest { ids }))
                .await
                .unwrap_err()
                .code(),
            Code::InvalidArgument
        );
    }
    env.api
        .delete_nv_link_domain(Request::new(lookup.clone()))
        .await
        .unwrap();
    let deleting = env
        .api
        .get_nv_link_domain(Request::new(lookup.clone()))
        .await
        .unwrap()
        .into_inner();
    assert!(deleting.status.as_ref().unwrap().deletion_requested);
    assert_ne!(deleting.version, domain.version);
    assert_eq!(deleting.config, domain.config);
    assert_eq!(
        env.api
            .create_nv_link_domain(Request::new(request))
            .await
            .unwrap_err()
            .code(),
        Code::AlreadyExists
    );
    env.api
        .delete_nv_link_domain(Request::new(lookup.clone()))
        .await
        .unwrap();
    assert_eq!(
        env.api
            .get_nv_link_domain(Request::new(lookup))
            .await
            .unwrap()
            .into_inner(),
        deleting
    );
}
