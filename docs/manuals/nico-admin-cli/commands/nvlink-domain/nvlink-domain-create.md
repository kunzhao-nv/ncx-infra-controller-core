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

Compute member; repeat for each tray (BMC MAC)

`--switch <SWITCH>`

Switch member; repeat for each tray (BMC MAC)

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
nico-admin-cli nvlink-domain create group-01 mini-profile --name mini-a --compute 00:11:22:33:44:55 --switch 00:11:22:33:44:66
```

---

**Related:** [Hardware commands](../../hardware.md) · [CLI reference index](../../README.md)
