/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use ::rpc::forge::{self as rpc, HealthReportEntry};
use carbide_uuid::nvlink::NvLinkDomainId;
use health_report::HealthReportApplyMode;
use model::health::HealthReportSources;
use tonic::{Request, Response, Status};

use crate::CarbideError;
use crate::api::{Api, log_request_data};
use crate::auth::AuthContext;

fn resource_id(value: Option<::rpc::common::Uuid>) -> Result<uuid::Uuid, CarbideError> {
    let value = value.ok_or(CarbideError::MissingArgument("id"))?;
    let id = uuid::Uuid::parse_str(&value.value)
        .map_err(|_| CarbideError::InvalidArgument("id must be a UUID".into()))?;
    if id.is_nil() {
        return Err(CarbideError::InvalidArgument("id must not be nil".into()));
    }
    Ok(id)
}

pub(crate) async fn create(
    api: &Api,
    request: Request<rpc::CreateNvLinkDomainRequest>,
) -> Result<Response<rpc::NvLinkDomain>, Status> {
    log_request_data(&request);
    let request = request.into_inner();
    let config: model::nvlink_domain::NvLinkDomainConfig = request
        .config
        .ok_or(CarbideError::MissingArgument("config"))?
        .try_into()
        .map_err(CarbideError::from)?;
    let metadata: model::metadata::Metadata = request
        .metadata
        .map(TryInto::try_into)
        .transpose()
        .map_err(CarbideError::from)?
        .unwrap_or_default();
    let profile = api
        .runtime_config
        .domain_profiles
        .get(&config.domain_profile_id)
        .ok_or_else(|| {
            CarbideError::InvalidArgument(format!(
                "domain profile {} is not configured",
                config.domain_profile_id
            ))
        })?;
    profile
        .validate_members(&config)
        .map_err(|error| CarbideError::InvalidArgument(error.to_string()))?;
    let id = uuid::Uuid::new_v4();
    let mut txn = api.txn_begin().await?;
    db::nvlink_domain::create(&mut txn, id, &config, &metadata, false)
        .await
        .map_err(CarbideError::from)?;
    let domain = db::nvlink_domain::find_by_ids(&mut txn, &[id])
        .await
        .map_err(CarbideError::from)?
        .pop()
        .ok_or_else(|| CarbideError::internal("created Domain is missing".into()))?;
    txn.commit().await?;
    Ok(Response::new(domain.into()))
}

pub(crate) async fn delete(
    api: &Api,
    request: Request<rpc::NvLinkDomainRequest>,
) -> Result<Response<()>, Status> {
    log_request_data(&request);
    let id = resource_id(request.into_inner().id)?;
    let mut txn = api.txn_begin().await?;
    db::nvlink_domain::request_deletion(&mut txn, id)
        .await
        .map_err(CarbideError::from)?;
    txn.commit().await?;
    Ok(Response::new(()))
}

pub(crate) async fn get(
    api: &Api,
    request: Request<rpc::NvLinkDomainRequest>,
) -> Result<Response<rpc::NvLinkDomain>, Status> {
    log_request_data(&request);
    let id = resource_id(request.into_inner().id)?;
    let domain = db::nvlink_domain::find_by_ids(api.db_reader().as_mut(), &[id])
        .await
        .map_err(CarbideError::from)?
        .pop()
        .ok_or_else(|| CarbideError::NotFoundError {
            kind: "nvlink_domain",
            id: id.to_string(),
        })?;
    Ok(Response::new(domain.into()))
}

pub(crate) async fn find_ids(
    api: &Api,
    request: Request<rpc::NvLinkDomainSearchFilter>,
) -> Result<Response<rpc::NvLinkDomainIdList>, Status> {
    log_request_data(&request);
    let group = request.into_inner().rack_group_id;
    if let Some(id) = &group {
        ::rpc::model::expected_rack_group::validate_rack_group_id(id)
            .map_err(CarbideError::from)?;
    }
    let ids = db::nvlink_domain::find_ids(api.db_reader().as_mut(), group.as_ref())
        .await
        .map_err(CarbideError::from)?;
    Ok(Response::new(rpc::NvLinkDomainIdList {
        ids: ids.into_iter().map(Into::into).collect(),
    }))
}

pub(crate) async fn find_by_ids(
    api: &Api,
    request: Request<rpc::NvLinkDomainsByIdsRequest>,
) -> Result<Response<rpc::NvLinkDomainList>, Status> {
    log_request_data(&request);
    let ids = request.into_inner().ids;
    let limit = api.runtime_config.max_find_by_ids as usize;
    if ids.is_empty() || ids.len() > limit {
        return Err(
            CarbideError::InvalidArgument(format!("provide between 1 and {limit} IDs")).into(),
        );
    }
    let ids = ids
        .into_iter()
        .map(|id| resource_id(Some(id)))
        .collect::<Result<Vec<_>, _>>()?;
    let domains = db::nvlink_domain::find_by_ids(api.db_reader().as_mut(), &ids)
        .await
        .map_err(CarbideError::from)?;
    Ok(Response::new(rpc::NvLinkDomainList {
        domains: domains.into_iter().map(Into::into).collect(),
    }))
}

pub(crate) async fn list_nv_link_domain_health_reports(
    api: &Api,
    request: Request<rpc::ListNvLinkDomainHealthReportsRequest>,
) -> Result<Response<rpc::ListHealthReportResponse>, Status> {
    log_request_data(&request);

    let domain_id = request
        .into_inner()
        .domain_id
        .ok_or_else(|| CarbideError::MissingArgument("domain_id"))?;

    let health_reports =
        db::nvlink_domain_health_report::find(api.db_reader().as_mut(), &domain_id)
            .await?
            .unwrap_or_default();

    Ok(Response::new(list_response(health_reports)))
}

pub(crate) async fn insert_nv_link_domain_health_report(
    api: &Api,
    request: Request<rpc::InsertNvLinkDomainHealthReportRequest>,
) -> Result<Response<()>, Status> {
    log_request_data(&request);

    let triggered_by = request
        .extensions()
        .get::<AuthContext>()
        .and_then(|ctx| ctx.get_external_user_name())
        .map(String::from);

    let rpc::InsertNvLinkDomainHealthReportRequest {
        domain_id,
        health_report_entry: Some(rpc::HealthReportEntry { report, mode }),
    } = request.into_inner()
    else {
        return Err(CarbideError::MissingArgument("health_report_entry").into());
    };

    let domain_id = domain_id.ok_or_else(|| CarbideError::MissingArgument("domain_id"))?;

    let Some(report) = report else {
        return Err(CarbideError::MissingArgument("report").into());
    };

    let Ok(mode) = rpc::HealthReportApplyMode::try_from(mode) else {
        return Err(CarbideError::InvalidArgument("mode".to_string()).into());
    };

    let mode: HealthReportApplyMode = mode.into();

    let mut txn = api.txn_begin().await?;
    let health_reports = db::nvlink_domain_health_report::find(&mut txn, &domain_id)
        .await?
        .unwrap_or_default();

    let mut report = health_report::HealthReport::try_from(report.clone())
        .map_err(|e| CarbideError::internal(e.to_string()))?;

    if report.observed_at.is_none() {
        report.observed_at = Some(chrono::Utc::now());
    }
    report.triggered_by = triggered_by;
    report.update_in_alert_since(health_reports.by_source(&report.source));

    match remove_by_source(&mut txn, &domain_id, &health_reports, report.source.clone()).await {
        Ok(_) | Err(CarbideError::NotFoundError { .. }) => {}
        Err(e) => return Err(e.into()),
    }

    db::nvlink_domain_health_report::insert_health_report(&mut txn, &domain_id, mode, &report)
        .await?;

    txn.commit().await?;

    Ok(Response::new(()))
}

pub(crate) async fn remove_nv_link_domain_health_report(
    api: &Api,
    request: Request<rpc::RemoveNvLinkDomainHealthReportRequest>,
) -> Result<Response<()>, Status> {
    log_request_data(&request);

    let rpc::RemoveNvLinkDomainHealthReportRequest { domain_id, source } = request.into_inner();
    let domain_id = domain_id.ok_or_else(|| CarbideError::MissingArgument("domain_id"))?;

    let mut txn = api.txn_begin().await?;
    let health_reports = db::nvlink_domain_health_report::find(&mut txn, &domain_id)
        .await?
        .unwrap_or_default();

    remove_by_source(&mut txn, &domain_id, &health_reports, source).await?;
    txn.commit().await?;

    Ok(Response::new(()))
}

async fn remove_by_source(
    txn: &mut db::Transaction<'_>,
    domain_id: &NvLinkDomainId,
    health_reports: &HealthReportSources,
    source: String,
) -> Result<(), CarbideError> {
    let mode = if health_reports.replace.as_ref().map(|o| &o.source) == Some(&source) {
        HealthReportApplyMode::Replace
    } else if health_reports.merges.contains_key(&source) {
        HealthReportApplyMode::Merge
    } else {
        return Err(CarbideError::NotFoundError {
            kind: "NVLink domain health report with source",
            id: source,
        });
    };

    db::nvlink_domain_health_report::remove_health_report(&mut *txn, domain_id, mode, &source)
        .await?;

    Ok(())
}

fn list_response(health_reports: HealthReportSources) -> rpc::ListHealthReportResponse {
    rpc::ListHealthReportResponse {
        health_report_entries: health_reports
            .into_iter()
            .map(|o| HealthReportEntry {
                report: Some(o.0.into()),
                mode: o.1 as i32,
            })
            .collect(),
    }
}
