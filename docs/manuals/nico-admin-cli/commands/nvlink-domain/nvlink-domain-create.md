# `nico-admin-cli nvlink-domain create`

*[Hardware commands](../../hardware.md) › [nvlink-domain](./nvlink-domain.md) › **create***

## NAME

nico-admin-cli-nvlink-domain-create - Declare a Domain and reserve
members for fabric reconciliation

## SYNOPSIS

```text
nico-admin-cli nvlink-domain create <--compute>
<--switch> [--name] [-h|--help]
<RACK_GROUP_ID> <DOMAIN_PROFILE_ID>
```

## DESCRIPTION

Declare a Domain and reserve members for fabric reconciliation

## OPTIONS

`--compute <COMPUTE>`

Compute member; repeat for each tray (RACK_ID:TRAY_INDEX)

`--switch <SWITCH>`

Switch member; repeat for each tray (RACK_ID:TRAY_INDEX)

`--name <NAME>`

Operator-facing name; omission stores an empty name

`-h, --help`

Print help (see a summary with -h)

`<RACK_GROUP_ID>`

Existing external RackGroup ID

`<DOMAIN_PROFILE_ID>`

Configured Domain Profile ID; case-sensitive

## Examples

```sh
nico-admin-cli nvlink-domain create group-01 mini-profile --name mini-a --compute rack-01:0 --switch rack-01:0
```

---

**Related:** [Hardware commands](../../hardware.md) · [CLI reference index](../../README.md)
