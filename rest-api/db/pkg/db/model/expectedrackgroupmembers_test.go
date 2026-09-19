// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

package model

import (
	"encoding/json"
	"testing"

	corev1 "github.com/NVIDIA/infra-controller/rest-api/proto/core/gen/v1"
	"github.com/stretchr/testify/require"
)

func TestExpectedRackGroupMembers_ToProto(t *testing.T) {
	for _, tc := range []struct {
		rest ExpectedRackGroupMemberType
		core string
	}{
		{ExpectedRackGroupMemberTypeCompute, "Compute"},
		{ExpectedRackGroupMemberTypeNVSwitch, "Switch"},
		{ExpectedRackGroupMemberTypePowerShelf, "PowerShelf"},
	} {
		t.Run(string(tc.rest), func(t *testing.T) {
			members := ExpectedRackGroupMembers{{Type: tc.rest, Manufacturer: "NVIDIA", ID: "device-01"}}
			require.NoError(t, members.Validate())
			wire := members.ToProto()
			require.Equal(t, tc.core, wire[0].Type)
			var result ExpectedRackGroupMembers
			require.NoError(t, result.FromProto(wire))
			require.Equal(t, members, result)
			stored, err := json.Marshal(result)
			require.NoError(t, err)
			require.JSONEq(t, `[{"type":"`+string(tc.rest)+`","manufacturer":"NVIDIA","id":"device-01"}]`, string(stored))
		})
	}
}

func TestExpectedRackGroupMembers_FromProto(t *testing.T) {
	for _, name := range []string{"", "switch", "NVSwitch", "NVSWITCH", "Other"} {
		t.Run(name, func(t *testing.T) {
			original := ExpectedRackGroupMembers{{Type: ExpectedRackGroupMemberTypeCompute, Manufacturer: "NVIDIA", ID: "original"}}
			members := append(ExpectedRackGroupMembers{}, original...)
			err := members.FromProto([]*corev1.ExpectedRackGroupMember{
				{Type: "Switch", Manufacturer: "NVIDIA", Id: "valid"},
				{Type: name, Manufacturer: "NVIDIA", Id: "invalid"},
			})
			require.Error(t, err)
			require.Equal(t, original, members)
		})
	}
}
