/*
 * SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::stream;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::{Bytes, Frame, Incoming};
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use prost::Message;
use rpc::forge_api_client::ForgeApiClient;
use rpc::forge_tls_client::{ApiConfig, ForgeClientConfig};
use tokio::net::TcpListener;

use super::*;
use crate::async_write::CapturedOutput;
use crate::cfg::cli_options::{CliCommand, CliOptions, SortField};
use crate::cfg::runtime::RuntimeConfig;
use crate::rpc::ApiClient;

fn parse(args: &[&str]) -> Result<Cmd, clap::Error> {
    let options = CliOptions::try_parse_from(
        ["nico-admin-cli", "nvlink-domain"]
            .into_iter()
            .chain(args.iter().copied()),
    )?;
    let Some(CliCommand::NvlinkDomain(command)) = options.commands else {
        panic!("expected domain command")
    };
    Ok(command)
}

#[tokio::test]
async fn command_dispatch_and_table_contract() {
    let id = "12345678-1234-5678-90ab-cdef01234567";
    let domain = forge::NvLinkDomain {
        id: Some(id.parse::<Uuid>().unwrap().into()),
        config: Some(forge::NvLinkDomainConfig {
            rack_group_id: Some("group".parse().unwrap()),
            domain_profile_id: "mini".into(),
            members: vec![],
        }),
        metadata: Some(forge::Metadata::default()),
        status: Some(forge::NvLinkDomainStatus {
            state: forge::NvLinkDomainLifecycleState::Discovering as i32,
            ..Default::default()
        }),
        ..Default::default()
    };
    let (output, calls) = dispatch(&["show", id], OutputFormat::AsciiTable, domain.clone()).await;
    for value in [
        "Domain ID",
        "Rack Group ID",
        "Name",
        "Profile",
        "State",
        "Members",
        id,
        "group",
        "mini",
        "Discovering",
    ] {
        assert!(output.contains(value), "{value}: {output}");
    }
    assert_eq!(calls[0].0, "/forge.Forge/GetNvLinkDomain");
    let (csv, _) = dispatch(&["show", id], OutputFormat::Csv, domain.clone()).await;
    assert!(csv.contains("group,,mini,Discovering,0"), "{csv}");

    let (_, calls) = dispatch(
        &[
            "create",
            "group",
            "MiNi",
            "--compute",
            "00:11:22:33:44:55",
            "--switch",
            "00:11:22:33:44:66",
        ],
        OutputFormat::Json,
        domain.clone(),
    )
    .await;
    assert_eq!(calls[0].0, "/forge.Forge/CreateNvLinkDomain");
    let request = forge::CreateNvLinkDomainRequest::decode(calls[0].1.as_slice()).unwrap();
    let config = request.config.unwrap();
    assert_eq!(config.domain_profile_id, "MiNi");
    assert_eq!(config.members.len(), 2);
    assert_eq!(
        config.members[0].component_type(),
        forge::NvLinkDomainComponentType::Compute
    );
    assert_eq!(
        config.members[1].component_type(),
        forge::NvLinkDomainComponentType::Switch
    );
    assert_eq!(config.members[0].bmc_mac_address, "00:11:22:33:44:55");
    let (_, calls) = dispatch(
        &["show", "--rack-group-id", "group"],
        OutputFormat::Json,
        domain.clone(),
    )
    .await;
    assert_eq!(calls[0].0, "/forge.Forge/FindNvLinkDomainIds");
    assert_eq!(calls[1].0, "/forge.Forge/FindNvLinkDomainsByIds");
    let (_, calls) = dispatch(&["delete", id], OutputFormat::Json, domain.clone()).await;
    assert_eq!(calls[0].0, "/forge.Forge/DeleteNvLinkDomain");
    assert_eq!(
        forge::NvLinkDomainRequest::decode(calls[0].1.as_slice())
            .unwrap()
            .id,
        domain.id
    );
}

async fn dispatch(
    args: &[&str],
    format: OutputFormat,
    domain: forge::NvLinkDomain,
) -> (String, Vec<(String, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let server_calls = calls.clone();
    let server = tokio::spawn(async move {
        let (connection, _) = listener.accept().await.unwrap();
        http2::Builder::new(TokioExecutor::new())
            .serve_connection(
                TokioIo::new(connection),
                service_fn(move |request: Request<Incoming>| {
                    let calls = server_calls.clone();
                    let domain = domain.clone();
                    async move {
                        let path = request.uri().path().to_string();
                        let body = request.into_body().collect().await.unwrap().to_bytes();
                        let encoded = if path == "/forge.Forge/Version" {
                            forge::BuildInfo::default().encode_to_vec()
                        } else {
                            calls
                                .lock()
                                .unwrap()
                                .push((path.clone(), body[5..].to_vec()));
                            match path.as_str() {
                                "/forge.Forge/CreateNvLinkDomain"
                                | "/forge.Forge/GetNvLinkDomain" => domain.encode_to_vec(),
                                "/forge.Forge/DeleteNvLinkDomain" => vec![],
                                "/forge.Forge/FindNvLinkDomainIds" => forge::NvLinkDomainIdList {
                                    ids: vec![domain.id.unwrap()],
                                }
                                .encode_to_vec(),
                                "/forge.Forge/FindNvLinkDomainsByIds" => forge::NvLinkDomainList {
                                    domains: vec![domain],
                                }
                                .encode_to_vec(),
                                _ => panic!("unexpected RPC {path}"),
                            }
                        };
                        let mut data = vec![0];
                        data.extend_from_slice(
                            &u32::try_from(encoded.len()).unwrap().to_be_bytes(),
                        );
                        data.extend(encoded);
                        let mut trailers = hyper::HeaderMap::new();
                        trailers
                            .insert("grpc-status", hyper::header::HeaderValue::from_static("0"));
                        let body = StreamBody::new(stream::iter([
                            Ok::<_, Infallible>(Frame::data(Bytes::from(data))),
                            Ok(Frame::trailers(trailers)),
                        ]))
                        .boxed_unsync();
                        Ok::<_, Infallible>(
                            Response::builder()
                                .header("content-type", "application/grpc+tonic")
                                .body(body)
                                .unwrap(),
                        )
                    }
                }),
            )
            .await
            .unwrap();
    });
    let config = ForgeClientConfig {
        request_timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    };
    let mut captured = CapturedOutput::new();
    let context = RuntimeContext {
        api_client: ApiClient(ForgeApiClient::new(&ApiConfig::new(
            &format!("http://{address}"),
            &config,
        ))),
        config: RuntimeConfig {
            format,
            request_timeout: config.request_timeout,
            page_size: 1,
            extended: false,
            cloud_unsafe_op: None,
            sort_by: SortField::PrimaryId,
        },
        output_file: std::mem::replace(captured.writer(), Box::new(tokio::io::sink())),
    };
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        parse(args).unwrap().dispatch(context),
    )
    .await;
    server.abort();
    if let Err(error) = server.await {
        assert!(error.is_cancelled(), "{error}");
    }
    result.unwrap().unwrap();
    let output = String::from_utf8(captured.into_bytes().await).unwrap();
    let calls = calls.lock().unwrap().clone();
    (output, calls)
}
