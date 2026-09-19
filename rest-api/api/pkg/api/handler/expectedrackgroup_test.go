// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

package handler

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	cdbm "github.com/NVIDIA/infra-controller/rest-api/db/pkg/db/model"
	"github.com/google/uuid"
	"github.com/labstack/echo/v4"
	"github.com/stretchr/testify/require"
)

func rackGroupValidationContext(method, body string) (echo.Context, *httptest.ResponseRecorder) {
	req := httptest.NewRequest(method, "/v2/org/test/nico/expected-rack-group", strings.NewReader(body))
	req.Header.Set(echo.HeaderContentType, echo.MIMEApplicationJSON)
	rec := httptest.NewRecorder()
	ctx := echo.New().NewContext(req, rec)
	ctx.Set("user", &cdbm.User{ID: uuid.New()})
	ctx.SetParamNames("orgName", "id")
	ctx.SetParamValues("test", "550e8400-e29b-41d4-a716-446655440000")
	return ctx, rec
}

func TestReplaceAllExpectedRackGroupsHandler_Handle(t *testing.T) {
	// Nil dependencies prove malformed replacement requests cannot reach DB or workflows.
	handler := NewReplaceAllExpectedRackGroupsHandler(nil, nil, nil)
	for _, body := range []string{
		`{"siteId":"550e8400-e29b-41d4-a716-446655440000"}`,
		`{"siteId":"550e8400-e29b-41d4-a716-446655440000","expectedRackGroups":null}`,
	} {
		t.Run(body, func(t *testing.T) {
			ctx, rec := rackGroupValidationContext(http.MethodPut, body)
			require.NoError(t, handler.Handle(ctx))
			require.Equal(t, http.StatusBadRequest, rec.Code)
			require.Contains(t, rec.Body.String(), "expectedRackGroups")
		})
	}
}

func TestCreateExpectedRackGroupHandler_Handle(t *testing.T) {
	ctx, rec := rackGroupValidationContext(http.MethodPost,
		`{"siteId":"550e8400-e29b-41d4-a716-446655440000","rackGroupId":"group","topology":"t","name":"机架"}`)
	require.NoError(t, NewCreateExpectedRackGroupHandler(nil, nil, nil).Handle(ctx))
	require.Equal(t, http.StatusBadRequest, rec.Code)
	require.Contains(t, rec.Body.String(), "name")
}

func TestUpdateExpectedRackGroupHandler_Handle(t *testing.T) {
	ctx, rec := rackGroupValidationContext(http.MethodPatch, `{"description":"`+strings.Repeat("d", 1025)+`"}`)
	require.NoError(t, NewUpdateExpectedRackGroupHandler(nil, nil, nil).Handle(ctx))
	require.Equal(t, http.StatusBadRequest, rec.Code)
	require.Contains(t, rec.Body.String(), "description")
}

func TestGetAllExpectedRackGroupHandler_Handle(t *testing.T) {
	ctx, rec := rackGroupValidationContext(http.MethodGet, "")
	query := ctx.Request().URL.Query()
	query.Set("siteId", "invalid-uuid")
	ctx.Request().URL.RawQuery = query.Encode()
	require.NoError(t, NewGetAllExpectedRackGroupHandler(nil, nil).Handle(ctx))
	require.Equal(t, http.StatusBadRequest, rec.Code)
	require.Contains(t, rec.Body.String(), "siteId")
}
