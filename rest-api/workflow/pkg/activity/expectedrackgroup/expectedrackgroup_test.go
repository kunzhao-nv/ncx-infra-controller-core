// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

package expectedrackgroup

import (
	"context"
	"testing"
	"time"

	cdb "github.com/NVIDIA/infra-controller/rest-api/db/pkg/db"
	cdbm "github.com/NVIDIA/infra-controller/rest-api/db/pkg/db/model"
	cdbu "github.com/NVIDIA/infra-controller/rest-api/db/pkg/util"
	corev1 "github.com/NVIDIA/infra-controller/rest-api/proto/core/gen/v1"
	cwu "github.com/NVIDIA/infra-controller/rest-api/workflow/pkg/util"
	"github.com/google/uuid"
	"github.com/stretchr/testify/require"
)

func TestManageExpectedRackGroup_UpdateExpectedRackGroupsInDB(t *testing.T) {
	ctx := context.Background()
	session := cdbu.GetTestDBSession(t, false)
	defer session.Close()
	for _, model := range []interface{}{(*cdbm.InfrastructureProvider)(nil), (*cdbm.Site)(nil), (*cdbm.User)(nil), (*cdbm.ExpectedRackGroup)(nil)} {
		require.NoError(t, session.DB.ResetModel(ctx, model))
	}
	user := cwu.TestBuildUser(t, session, uuid.NewString(), []string{"group-test"}, []string{"FORGE_PROVIDER_ADMIN"})
	provider := cwu.TestBuildInfrastructureProvider(t, session, "provider", "group-test", user)
	site := cwu.TestBuildSite(t, session, provider, "site", cdbm.SiteStatusRegistered, nil, user)
	dao := cdbm.NewExpectedRackGroupDAO(session)
	manager := NewManageExpectedRackGroup(session, nil)

	for _, tc := range []struct {
		name      string
		inventory *corev1.ExpectedRackGroupInventory
		recent    bool
		deleted   bool
		wantErr   bool
	}{
		{name: "missing snapshot", wantErr: true},
		{name: "unspecified status preserves inventory", inventory: &corev1.ExpectedRackGroupInventory{}, wantErr: true},
		{name: "failed snapshot preserves inventory", inventory: &corev1.ExpectedRackGroupInventory{InventoryStatus: corev1.InventoryStatus_INVENTORY_STATUS_FAILED}},
		{name: "successful empty snapshot deletes old rows", inventory: &corev1.ExpectedRackGroupInventory{InventoryStatus: corev1.InventoryStatus_INVENTORY_STATUS_SUCCESS}, deleted: true},
		{name: "recent API write survives empty snapshot", inventory: &corev1.ExpectedRackGroupInventory{InventoryStatus: corev1.InventoryStatus_INVENTORY_STATUS_SUCCESS}, recent: true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			row, err := dao.Create(ctx, nil, cdbm.ExpectedRackGroupCreateInput{
				ExpectedRackGroupID: uuid.New(), SiteID: site.ID, RackGroupID: tc.name, Topology: "topology", CreatedBy: user.ID,
			})
			require.NoError(t, err)
			if !tc.recent {
				_, err = session.DB.NewUpdate().Model((*cdbm.ExpectedRackGroup)(nil)).
					Set("updated = ?", time.Now().Add(-time.Hour)).Where("id = ?", row.ID).Exec(ctx)
				require.NoError(t, err)
			}
			err = manager.UpdateExpectedRackGroupsInDB(ctx, site.ID, tc.inventory)
			if tc.wantErr {
				require.Error(t, err)
			} else {
				require.NoError(t, err)
			}
			_, err = dao.Get(ctx, nil, row.ID, nil, false)
			if tc.deleted {
				require.ErrorIs(t, err, cdb.ErrDoesNotExist)
			} else {
				require.NoError(t, err)
				require.NoError(t, dao.Delete(ctx, nil, row.ID))
			}
		})
	}
}
