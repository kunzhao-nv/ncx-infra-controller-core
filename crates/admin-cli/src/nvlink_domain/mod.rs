/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

use carbide_uuid::rack::RackGroupId;
use clap::Parser;
use color_eyre::eyre::WrapErr;
use prettytable::{Table, row};
use rpc::admin_cli::OutputFormat;
use rpc::forge;
use uuid::Uuid;

use crate::cfg::dispatch::Dispatch;
use crate::cfg::run::Run;
use crate::cfg::runtime::RuntimeContext;
use crate::errors::CarbideCliResult;
use crate::{async_write, async_write_table_as_csv};

#[cfg(test)]
mod tests;

#[derive(Parser, Debug, Dispatch)]
#[command(defer = |cmd| cmd.mut_args(|arg| match arg.get_id().as_str() {
    "extended" | "sort_by" => arg.hide(true),
    _ => arg,
}))]
pub(crate) enum Cmd {
    /// Declare a Domain and reserve members for fabric reconciliation
    Create(Create),
    /// Show one Domain or list Domains in UUID order
    Show(Show),
    /// Request teardown; members remain reserved until cleanup completes
    Delete(Delete),
}

#[derive(Clone, Debug)]
struct MemberSelector {
    rack_id: carbide_uuid::rack::RackId,
    tray_index: i32,
}

impl std::str::FromStr for MemberSelector {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (rack, index) = value.rsplit_once(':').ok_or("use RACK_ID:TRAY_INDEX")?;
        let rack_id = rack.parse().map_err(|error| format!("{error}"))?;
        let tray_index = index
            .parse::<i32>()
            .map_err(|_| "tray index must be a number")?;
        if tray_index < 0 {
            return Err("tray index must be non-negative".into());
        }
        Ok(Self {
            rack_id,
            tray_index,
        })
    }
}

fn member(
    selector: MemberSelector,
    component_type: forge::NvLinkDomainComponentType,
) -> forge::NvLinkDomainMember {
    forge::NvLinkDomainMember {
        component_type: component_type as i32,
        rack_id: Some(selector.rack_id),
        tray_index: Some(selector.tray_index),
    }
}

#[derive(Parser, Debug)]
#[command(after_long_help = "\
EXAMPLES:

Create a Domain using a configured profile:
    $ nico-admin-cli nvlink-domain create group-01 mini-profile --name mini-a \
    --compute rack-01:0 --switch rack-01:0

")]
pub(crate) struct Create {
    /// Existing external RackGroup ID
    rack_group_id: RackGroupId,
    /// Configured Domain Profile ID; case-sensitive
    domain_profile_id: String,
    /// Compute member; repeat for each tray (RACK_ID:TRAY_INDEX)
    #[arg(long, required = true)]
    compute: Vec<MemberSelector>,
    /// Switch member; repeat for each tray (RACK_ID:TRAY_INDEX)
    #[arg(long, required = true)]
    switch: Vec<MemberSelector>,
    /// Operator-facing name; omission stores an empty name
    #[arg(long)]
    name: Option<String>,
}

impl Run for Create {
    async fn run(self, ctx: &mut RuntimeContext) -> CarbideCliResult<()> {
        let members = self
            .compute
            .into_iter()
            .map(|value| member(value, forge::NvLinkDomainComponentType::Compute))
            .chain(
                self.switch
                    .into_iter()
                    .map(|value| member(value, forge::NvLinkDomainComponentType::Switch)),
            )
            .collect();
        let domain = ctx
            .api_client
            .0
            .create_nv_link_domain(forge::CreateNvLinkDomainRequest {
                config: Some(forge::NvLinkDomainConfig {
                    rack_group_id: Some(self.rack_group_id),
                    domain_profile_id: self.domain_profile_id,
                    members,
                }),
                metadata: Some(forge::Metadata {
                    name: self.name.unwrap_or_default(),
                    ..Default::default()
                }),
            })
            .await
            .wrap_err("creating NVLink domain")?;
        render(
            ctx,
            forge::NvLinkDomainList {
                domains: vec![domain],
            },
        )
        .await
    }
}

#[derive(Parser, Debug)]
#[command(after_long_help = "\
EXAMPLES:

List Domains:
    $ nico-admin-cli nvlink-domain show

Filter by RackGroup:
    $ nico-admin-cli nvlink-domain show --rack-group-id group-01

Show one Domain:
    $ nico-admin-cli nvlink-domain show 12345678-1234-5678-90ab-cdef01234567

")]
pub(crate) struct Show {
    /// Stable Domain UUID; omission lists all
    id: Option<Uuid>,
    /// Restrict listing to a physical RackGroup
    #[arg(long, conflicts_with = "id")]
    rack_group_id: Option<RackGroupId>,
}

impl Run for Show {
    async fn run(self, ctx: &mut RuntimeContext) -> CarbideCliResult<()> {
        let mut result = forge::NvLinkDomainList::default();
        if let Some(id) = self.id {
            result.domains.push(
                ctx.api_client
                    .0
                    .get_nv_link_domain(forge::NvLinkDomainRequest {
                        id: Some(id.into()),
                    })
                    .await
                    .wrap_err("getting NVLink domain")?,
            );
        } else {
            let ids = ctx
                .api_client
                .0
                .find_nv_link_domain_ids(forge::NvLinkDomainSearchFilter {
                    rack_group_id: self.rack_group_id,
                })
                .await
                .wrap_err("listing NVLink domain IDs")?;
            let page_size = ctx
                .api_client
                .effective_chunk_size(ctx.config.page_size.max(1))
                .await?;
            for ids in ids.ids.chunks(page_size) {
                result.domains.extend(
                    ctx.api_client
                        .0
                        .find_nv_link_domains_by_ids(forge::NvLinkDomainsByIdsRequest {
                            ids: ids.to_vec(),
                        })
                        .await
                        .wrap_err("loading NVLink domains")?
                        .domains,
                );
            }
        }
        render(ctx, result).await
    }
}

#[derive(Parser, Debug)]
#[command(after_long_help = "\
EXAMPLES:

Request Domain teardown:
    $ nico-admin-cli nvlink-domain delete 12345678-1234-5678-90ab-cdef01234567

")]
pub(crate) struct Delete {
    /// Stable Domain UUID. Deletion is asynchronous; repeated requests are safe.
    id: Uuid,
}

impl Run for Delete {
    async fn run(self, ctx: &mut RuntimeContext) -> CarbideCliResult<()> {
        ctx.api_client
            .0
            .delete_nv_link_domain(forge::NvLinkDomainRequest {
                id: Some(self.id.into()),
            })
            .await
            .wrap_err("requesting NVLink domain deletion")?;
        Ok(())
    }
}

async fn render(
    ctx: &mut RuntimeContext,
    domains: forge::NvLinkDomainList,
) -> CarbideCliResult<()> {
    match ctx.config.format {
        OutputFormat::Json => {
            let value =
                serde_json::to_string_pretty(&domains).wrap_err("serializing NVLink domains")?;
            async_write!(ctx.output_file, "{value}\n").wrap_err("writing NVLink domains")?;
        }
        OutputFormat::Yaml => {
            let value = serde_yaml::to_string(&domains).wrap_err("serializing NVLink domains")?;
            async_write!(ctx.output_file, "{value}").wrap_err("writing NVLink domains")?;
        }
        OutputFormat::AsciiTable | OutputFormat::Csv => {
            let mut table = Table::new();
            table.set_titles(row![
                "Domain ID",
                "Rack Group ID",
                "Name",
                "Profile",
                "State",
                "Members"
            ]);
            for domain in domains.domains {
                let config = domain.config.unwrap_or_default();
                let status = domain.status.unwrap_or_default();
                table.add_row(row![
                    domain.id.map(|id| id.value).unwrap_or_default(),
                    config
                        .rack_group_id
                        .map(|id| id.to_string())
                        .unwrap_or_default(),
                    domain.metadata.map(|value| value.name).unwrap_or_default(),
                    config.domain_profile_id,
                    format!("{:?}", status.state()),
                    config.members.len(),
                ]);
            }
            if ctx.config.format == OutputFormat::Csv {
                async_write_table_as_csv!(ctx.output_file, table)
                    .wrap_err("writing NVLink domain CSV")?;
            } else {
                async_write!(ctx.output_file, "{table}").wrap_err("writing NVLink domain table")?;
            }
        }
    }
    Ok(())
}
