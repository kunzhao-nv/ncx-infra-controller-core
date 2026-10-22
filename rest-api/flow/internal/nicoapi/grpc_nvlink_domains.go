// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

package nicoapi

import (
	"context"
	"fmt"

	corev1 "github.com/NVIDIA/infra-controller/rest-api/proto/core/gen/v1"
)

// GetNVLinkDomains reads every declared Domain, including pending deletion.
// One grpcTimeout budget covers ID discovery, limit discovery and all detail
// batches. A missing detail invalidates the entire snapshot; callers must not
// interpret an incomplete read as a deletion.
func (c *grpcClient) GetNVLinkDomains(ctx context.Context) ([]*corev1.NvLinkDomain, error) {
	ctx, cancel := context.WithTimeout(ctx, c.grpcTimeout)
	defer cancel()
	response, err := c.gclient.FindNvLinkDomainIds(ctx, &corev1.NvLinkDomainSearchFilter{})
	if err != nil {
		return nil, fmt.Errorf("list Core NVLink domain IDs: %w", err)
	}
	if response == nil {
		return nil, fmt.Errorf("list Core NVLink domain IDs: empty response")
	}
	ids := make([]string, 0, len(response.GetIds()))
	for _, id := range response.GetIds() {
		ids = append(ids, id.GetValue())
	}
	domains := make([]*corev1.NvLinkDomain, 0, len(ids))
	err = visitFindByIDBatches(ctx, "FindNvLinkDomainsByIds", c.gclient.loadMaxFindByIDs, ids,
		func(ctx context.Context, batch []string) ([]*corev1.NvLinkDomain, error) {
			if ctx.Err() != nil {
				return nil, ctx.Err()
			}
			request := &corev1.NvLinkDomainsByIdsRequest{Ids: make([]*corev1.UUID, 0, len(batch))}
			for _, id := range batch {
				request.Ids = append(request.Ids, &corev1.UUID{Value: id})
			}
			result, err := c.gclient.FindNvLinkDomainsByIds(ctx, request)
			if err != nil {
				return nil, err
			}
			return result.GetDomains(), nil
		},
		func(domain *corev1.NvLinkDomain) string { return domain.GetId().GetValue() },
		func(batch []*corev1.NvLinkDomain) error {
			domains = append(domains, batch...)
			return nil
		})
	if err != nil {
		return nil, fmt.Errorf("read Core NVLink domains: %w", err)
	}
	return domains, nil
}
