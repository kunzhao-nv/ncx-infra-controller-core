// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

package nicoapi

import (
	"context"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"

	corev1 "github.com/NVIDIA/infra-controller/rest-api/proto/core/gen/v1"
)

type domainSnapshotForgeClient struct {
	corev1.ForgeClient
	ids           []*corev1.UUID
	nilIDs        bool
	idsErr        error
	limit         uint32
	omitLast      bool
	expireAtLimit bool
	deadlines     []time.Time
	batches       [][]*corev1.UUID
}

func (c *domainSnapshotForgeClient) FindNvLinkDomainIds(ctx context.Context, _ *corev1.NvLinkDomainSearchFilter, _ ...grpc.CallOption) (*corev1.NvLinkDomainIdList, error) {
	deadline, _ := ctx.Deadline()
	c.deadlines = append(c.deadlines, deadline)
	if c.nilIDs || c.idsErr != nil {
		return nil, c.idsErr
	}
	return &corev1.NvLinkDomainIdList{Ids: c.ids}, nil
}

func (c *domainSnapshotForgeClient) Version(ctx context.Context, _ *corev1.VersionRequest, _ ...grpc.CallOption) (*corev1.BuildInfo, error) {
	deadline, _ := ctx.Deadline()
	c.deadlines = append(c.deadlines, deadline)
	if c.expireAtLimit {
		<-ctx.Done()
		return nil, ctx.Err()
	}
	return &corev1.BuildInfo{RuntimeConfig: &corev1.RuntimeConfig{MaxFindByIds: c.limit}}, nil
}

func (c *domainSnapshotForgeClient) FindNvLinkDomainsByIds(ctx context.Context, req *corev1.NvLinkDomainsByIdsRequest, _ ...grpc.CallOption) (*corev1.NvLinkDomainList, error) {
	deadline, _ := ctx.Deadline()
	c.deadlines = append(c.deadlines, deadline)
	c.batches = append(c.batches, req.GetIds())
	domains := make([]*corev1.NvLinkDomain, 0, len(req.GetIds()))
	for _, id := range req.GetIds() {
		if c.omitLast && id.GetValue() == c.ids[len(c.ids)-1].GetValue() {
			continue
		}
		domains = append(domains, &corev1.NvLinkDomain{
			Id:      id,
			Version: "version-1",
			Status:  &corev1.NvLinkDomainStatus{DeletionRequested: true},
		})
	}
	return &corev1.NvLinkDomainList{Domains: domains}, nil
}

func TestGrpcClient_GetNVLinkDomains(t *testing.T) {
	ids := []*corev1.UUID{
		{Value: "00000000-0000-4000-8000-000000000001"},
		{Value: "00000000-0000-4000-8000-000000000002"},
		{Value: "00000000-0000-4000-8000-000000000003"},
	}
	cases := []struct {
		name        string
		server      domainSnapshotForgeClient
		wantErr     string
		wantBatches int
		wantCount   int
		timeout     time.Duration
	}{
		{name: "empty inventory skips limit and details", wantCount: 0},
		{name: "server limit with final short batch", server: domainSnapshotForgeClient{ids: ids, limit: 2}, wantBatches: 2, wantCount: 3},
		{name: "late missing detail discards earlier batches", server: domainSnapshotForgeClient{ids: ids, limit: 2, omitLast: true}, wantErr: "incomplete response", wantBatches: 2},
		{name: "nil response is not an empty inventory", server: domainSnapshotForgeClient{nilIDs: true}, wantErr: "empty response"},
		{name: "duplicate IDs rejected", server: domainSnapshotForgeClient{ids: []*corev1.UUID{ids[0], ids[0]}}, wantErr: "duplicate ID"},
		{name: "invalid mixed ID list rejected", server: domainSnapshotForgeClient{ids: []*corev1.UUID{ids[0], nil}}, wantErr: "empty ID"},
		{name: "older Core remains distinguishable", server: domainSnapshotForgeClient{idsErr: status.Error(codes.Unimplemented, "unsupported")}, wantErr: "Unimplemented"},
		{name: "limit discovery consumes the shared deadline", server: domainSnapshotForgeClient{ids: ids, expireAtLimit: true}, timeout: 10 * time.Millisecond, wantErr: "context deadline exceeded"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			timeout := tc.timeout
			if timeout == 0 {
				timeout = time.Minute
			}
			client := &grpcClient{gclient: newBatchingForgeClient(&tc.server), grpcTimeout: timeout}
			got, err := client.GetNVLinkDomains(context.Background())
			require.Len(t, tc.server.batches, tc.wantBatches)
			for _, deadline := range tc.server.deadlines {
				require.Equal(t, tc.server.deadlines[0], deadline)
			}
			if tc.wantErr != "" {
				require.ErrorContains(t, err, tc.wantErr)
				require.Nil(t, got)
				if tc.server.idsErr != nil {
					require.Equal(t, codes.Unimplemented, status.Code(err))
				}
				return
			}
			require.NoError(t, err)
			require.NotNil(t, got)
			require.Len(t, got, tc.wantCount)
			if tc.wantCount == 0 {
				require.Len(t, tc.server.deadlines, 1)
			}
			for i, domain := range got {
				require.Equal(t, ids[i], domain.GetId())
				require.True(t, domain.GetStatus().GetDeletionRequested())
				require.Equal(t, "version-1", domain.GetVersion())
			}
		})
	}
}
