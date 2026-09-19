// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

package activity

import (
	"context"
	"errors"
	"fmt"
	"testing"

	corev1 "github.com/NVIDIA/infra-controller/rest-api/proto/core/gen/v1"
	"github.com/stretchr/testify/require"
	"google.golang.org/grpc"
)

type expectedRackGroupClient struct {
	corev1.ForgeClient
	count         int
	cap           uint32
	calls         int
	sizes         []int
	failAt        int
	emptyFirst    bool
	cancelVersion context.CancelFunc
}

func (c *expectedRackGroupClient) FindExpectedRackGroupIds(context.Context, *corev1.ExpectedRackGroupSearchFilter, ...grpc.CallOption) (*corev1.ExpectedRackGroupIdList, error) {
	result := &corev1.ExpectedRackGroupIdList{}
	for i := 0; i < c.count; i++ {
		result.RackGroupIds = append(result.RackGroupIds, &corev1.RackGroupId{Id: fmt.Sprintf("group-%03d", i)})
	}
	return result, nil
}

func (c *expectedRackGroupClient) Version(context.Context, *corev1.VersionRequest, ...grpc.CallOption) (*corev1.BuildInfo, error) {
	if c.cancelVersion != nil {
		c.cancelVersion()
	}
	return &corev1.BuildInfo{RuntimeConfig: &corev1.RuntimeConfig{MaxFindByIds: c.cap}}, nil
}

func (c *expectedRackGroupClient) FindExpectedRackGroupsByIds(ctx context.Context, request *corev1.ExpectedRackGroupsByIdsRequest, _ ...grpc.CallOption) (*corev1.ExpectedRackGroupList, error) {
	c.calls++
	c.sizes = append(c.sizes, len(request.RackGroupIds))
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	if c.calls == c.failAt {
		return nil, errors.New("batch unavailable")
	}
	result := &corev1.ExpectedRackGroupList{}
	if c.emptyFirst && c.calls == 1 {
		return result, nil
	}
	for _, id := range request.RackGroupIds {
		result.ExpectedRackGroups = append(result.ExpectedRackGroups, &corev1.ExpectedRackGroup{RackGroupId: id})
	}
	return result, nil
}

func TestCollectExpectedRackGroups(t *testing.T) {
	for _, tc := range []struct {
		name          string
		count         int
		cap           uint32
		emptyFirst    bool
		failAt        int
		cancelVersion bool
		wantSizes     []int
		wantCount     int
	}{
		{name: "empty inventory"},
		{name: "single record", count: 1, wantSizes: []int{1}, wantCount: 1},
		{name: "final partial batch", count: 26, wantSizes: []int{25, 1}, wantCount: 26},
		{name: "server cap", count: 3, cap: 1, wantSizes: []int{1, 1, 1}, wantCount: 3},
		{name: "deleted first batch does not end enumeration", count: 26, emptyFirst: true, wantSizes: []int{25, 1}, wantCount: 1},
		{name: "failed later batch discards partial snapshot", count: 26, failAt: 2, wantSizes: []int{25, 1}},
		{name: "earlier RPC exhausts shared context", count: 1, cancelVersion: true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			ctx, cancel := context.WithCancel(context.Background())
			defer cancel()
			client := &expectedRackGroupClient{count: tc.count, cap: tc.cap, failAt: tc.failAt, emptyFirst: tc.emptyFirst}
			if tc.cancelVersion {
				client.cancelVersion = cancel
			}
			result, err := collectExpectedRackGroups(ctx, client)
			if tc.failAt > 0 || tc.cancelVersion {
				require.Error(t, err)
				require.Nil(t, result)
				if tc.cancelVersion {
					require.ErrorIs(t, err, context.Canceled)
				}
			} else {
				require.NoError(t, err)
				require.Len(t, result.GetExpectedRackGroups(), tc.wantCount)
				if tc.wantCount > 0 {
					require.Equal(t, fmt.Sprintf("group-%03d", tc.count-1), result.ExpectedRackGroups[tc.wantCount-1].GetRackGroupId().GetId())
				}
			}
			require.Equal(t, tc.wantSizes, client.sizes)
		})
	}
}
