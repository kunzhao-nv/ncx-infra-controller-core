# `nico-admin-cli nvlink-domain delete`

*[Hardware commands](../../hardware.md) › [nvlink-domain](./nvlink-domain.md) › **delete***

## NAME

nico-admin-cli-nvlink-domain-delete - Request teardown; members remain
reserved until cleanup completes

## SYNOPSIS

```text
nico-admin-cli nvlink-domain delete [-h|--help] <ID>
```

## DESCRIPTION

Request teardown; members remain reserved until cleanup completes

## OPTIONS

`-h, --help`

Print help (see a summary with -h)

`<ID>`

Stable Domain UUID. Deletion is asynchronous; repeated requests are safe

## Examples

```sh
nico-admin-cli nvlink-domain delete 12345678-1234-5678-90ab-cdef01234567
```

---

**Related:** [Hardware commands](../../hardware.md) · [CLI reference index](../../README.md)
