// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

package model

import (
	"fmt"
	"strings"

	corev1 "github.com/NVIDIA/infra-controller/rest-api/proto/core/gen/v1"
)

// ExpectedRackGroupMemberType uses the REST rack component names.
type ExpectedRackGroupMemberType string

const (
	ExpectedRackGroupMemberTypeCompute    ExpectedRackGroupMemberType = "Compute"
	ExpectedRackGroupMemberTypeNVSwitch   ExpectedRackGroupMemberType = "NVSwitch"
	ExpectedRackGroupMemberTypePowerShelf ExpectedRackGroupMemberType = "PowerShelf"
)

// ExpectedRackGroupMember declares a device by its external identity.
type ExpectedRackGroupMember struct {
	Type         ExpectedRackGroupMemberType `json:"type"`
	Manufacturer string                      `json:"manufacturer"`
	ID           string                      `json:"id"`
}

// ExpectedRackGroupMembers is the device inventory declared for a group.
type ExpectedRackGroupMembers []ExpectedRackGroupMember

// Validate rejects incomplete identities and repeated device declarations.
func (members ExpectedRackGroupMembers) Validate() error {
	seen := make(map[ExpectedRackGroupMember]bool, len(members))
	for _, member := range members {
		switch member.Type {
		case ExpectedRackGroupMemberTypeCompute, ExpectedRackGroupMemberTypeNVSwitch, ExpectedRackGroupMemberTypePowerShelf:
		default:
			return fmt.Errorf("member type must be Compute, NVSwitch or PowerShelf")
		}
		if strings.TrimSpace(member.Manufacturer) == "" || strings.TrimSpace(member.ID) == "" {
			return fmt.Errorf("member type, manufacturer and id must not be blank")
		}
		if seen[member] {
			return fmt.Errorf("duplicate device member %q", member.ID)
		}
		seen[member] = true
	}
	return nil
}

// ToProto converts the declared device inventory for Core.
func (members ExpectedRackGroupMembers) ToProto() []*corev1.ExpectedRackGroupMember {
	result := make([]*corev1.ExpectedRackGroupMember, 0, len(members))
	for _, m := range members {
		coreType := string(m.Type)
		if m.Type == ExpectedRackGroupMemberTypeNVSwitch {
			coreType = "Switch"
		}
		result = append(result, &corev1.ExpectedRackGroupMember{Type: coreType, Manufacturer: m.Manufacturer, Id: m.ID})
	}
	return result
}

// FromProto replaces the device inventory, including an explicitly empty snapshot.
func (members *ExpectedRackGroupMembers) FromProto(value []*corev1.ExpectedRackGroupMember) error {
	converted := make(ExpectedRackGroupMembers, 0, len(value))
	for _, m := range value {
		var memberType ExpectedRackGroupMemberType
		switch m.GetType() {
		case "Compute":
			memberType = ExpectedRackGroupMemberTypeCompute
		case "Switch":
			memberType = ExpectedRackGroupMemberTypeNVSwitch
		case "PowerShelf":
			memberType = ExpectedRackGroupMemberTypePowerShelf
		default:
			return fmt.Errorf("unsupported Core rack group member type %q", m.GetType())
		}
		converted = append(converted, ExpectedRackGroupMember{Type: memberType, Manufacturer: m.GetManufacturer(), ID: m.GetId()})
	}
	*members = converted
	return nil
}
