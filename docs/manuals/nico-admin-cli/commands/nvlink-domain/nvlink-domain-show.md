# `nico-admin-cli nvlink-domain show`

*[Hardware commands](../../hardware.md) › [nvlink-domain](./nvlink-domain.md) › **show***

## NAME

nico-admin-cli-nvlink-domain-show - Show one Domain or list Domains in
UUID order

## SYNOPSIS

```text
nico-admin-cli nvlink-domain show [--rack-group-id]
[-h|--help] [ID]
```

## DESCRIPTION

Show one Domain or list Domains in UUID order

## OPTIONS

`--rack-group-id <RACK_GROUP_ID>`

Restrict listing to a physical RackGroup

`-h, --help`

Print help (see a summary with -h)

[*ID*]

Stable Domain UUID; omission lists all

## Examples

```sh
nico-admin-cli nvlink-domain show
nico-admin-cli nvlink-domain show --rack-group-id group-01
nico-admin-cli nvlink-domain show 12345678-1234-5678-90ab-cdef01234567
```

---

**Related:** [Hardware commands](../../hardware.md) · [CLI reference index](../../README.md)
