# Leakage REST API Options

## Goals

The API should answer four questions:

- Which event rule handles leakage, and where is it enabled or bound?
- Is a tray leaking, and what handling has occurred?
- Is a rack leaking, which trays originated the leak, and which trays are affected?
- Which racks and trays are leaking across a site?

Paths below are abbreviated proposals under `/v2/org/{org}/nico`. Site
selection, authorization, request bodies, and error mapping remain to be
defined with the final REST contract.

## Event rule management

Leakage should use the generic event-rule API with
`hardware.leak.detected` as its event type.

```http
GET    /event-rule
POST   /event-rule
GET    /event-rule/{ruleId}
PATCH  /event-rule/{ruleId}
DELETE /event-rule/{ruleId}
POST   /event-rule/{ruleId}/enable
POST   /event-rule/{ruleId}/disable
PUT    /event-rule/{ruleId}/binding/site
DELETE /event-rule/{ruleId}/binding/site
PUT    /event-rule/{ruleId}/binding/rack/{rackId}
DELETE /event-rule/{ruleId}/binding/rack/{rackId}
```

The built-in leakage rule is read-only and acts as the fallback. A custom rule
is created disabled, then bound and enabled. Resolution order is rack binding,
site binding, then the built-in rule.

## Tray API options

| Option | Benefit | Limitation | Decision |
| --- | --- | --- | --- |
| Add `leakStatus` to `GET /tray/{id}` | Cheap for lists and normal inventory reads | Cannot carry sensor or handling details | Use for summary |
| `GET /tray/{id}/leakage` | Clear domain model; supports sensors, alerts, rules, and tasks | Adds a dedicated endpoint | Use for details |
| Generic `/tray/{id}/state` | Could support other state types | Query/body semantics and response shape become ambiguous | Do not use |
| Generic `/tray/{id}/sensor-state` | Fits raw sensor data | Does not represent event rules or handling | Do not use as the leakage API |

Recommended combination:

```http
GET /tray/{id}
GET /tray/{id}/leakage
```

The tray resource exposes a small enum, not a free-form value:

```json
{
  "leakStatus": "Leaking"
}
```

Supported values are `Unknown`, `NoLeak`, and `Leaking`.

The detail response represents the current occurrence, or the latest occurrence
when no leak is active:

```json
{
  "trayId": "tray-1",
  "rackId": "rack-1",
  "status": "Leaking",
  "startedAt": "2026-09-14T18:30:00Z",
  "lastObservedAt": "2026-09-14T18:31:00Z",
  "clearedAt": null,
  "sensors": [],
  "position": {},
  "relatedAlerts": [],
  "appliedRule": {
    "id": "rule-id",
    "name": "Default leakage response"
  },
  "handling": {
    "status": "Running",
    "tasks": [
      {
        "id": "task-id",
        "operation": "ForcePowerOff",
        "status": "Running",
        "summary": "Powering off affected trays"
      }
    ]
  }
}
```

Handling status should be an enum such as `NotRequired`, `Pending`, `Running`,
`Succeeded`, `Failed`, or `PartiallySucceeded`; a Boolean cannot represent
in-progress or partial outcomes.

## Rack API

Use the same summary-plus-detail pattern:

```http
GET /rack/{id}
GET /rack/{id}/leakage
```

`GET /rack/{id}` adds `leakStatus`. The detail response contains rack-level
sensor information, `leakingTrays`, and `affectedTrays`. These tray sets are
different: an affected tray may be shut down because of its position without
reporting a leak itself.

Rack-level BMS leakage information must remain `Unknown` when NICo Core cannot
provide it. It must not be inferred as `NoLeak` from tray data.

## Site API options

| Option | Best use | Limitation | Decision |
| --- | --- | --- | --- |
| `GET /event/leakage` | Leakage-specific event history | Adds a special route per event category | Do not use |
| `GET /event?category=leakage` | Generic, paginated event history | Not an efficient current-state view | Use for history |
| `GET /leakage` | Current site-wide rack and tray leakage | Leakage-specific resource | Use for current state |
| `GET /rack/leakage` | Collection of leaking racks | Duplicates the site view and omits tray-only use cases | Add only if a separate rack collection is required |

Recommended endpoints:

```http
GET /leakage
GET /event?category=leakage
GET /event/{eventId}
```

`GET /leakage` returns the current site snapshot: leaking racks, leaking trays,
affected trays, and handling summaries. The event API returns historical
occurrences with event ID, timestamps, applied rule, rack, trays, and tasks.
It should expose a stable REST model rather than raw Flow database rows.

## Open questions

- Can NICo Core report rack-level BMS sensor state, location, and observation
  time?
- Should a cleared tray detail response include only the latest occurrence or
  no occurrence?
- What are the event retention and pagination limits?
- Should site-level rule binding be part of the first release?
