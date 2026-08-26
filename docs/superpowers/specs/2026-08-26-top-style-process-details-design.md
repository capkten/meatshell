# Top-Style Process Details

## Goal

Expand the detached process monitor so each row exposes the same operational
fields users expect from `top`: `PID USER PR NI VIRT RES SHR S %CPU %MEM TIME+
COMMAND`.

## Design

The remote process monitor command will request fixed-width-independent `ps`
fields in this order: `pid,user,pri,ni,vsz,rss,shr,stat,pcpu,pmem,time,args`.
The parser will consume the first eleven whitespace-delimited fields and join
the remaining fields into the command text. Existing bounds, two-second
sampling, sorting, paging, sidebar summary, and process termination behavior
remain unchanged.

`ProcInfo` will carry the additional values as display-ready strings where the
source format is host-dependent (`PR`, `NI`, `VIRT`, `RES`, `SHR`, `S`, and
`TIME+`) and retain numeric CPU/memory percentages for sorting and the load
bar. `ProcRow` will mirror those fields for Slint. The sidebar continues to
render its compact five-column summary; only the detached process window gets
the complete top-style table.

The detached window will use the exact top column order, compact widths, and
horizontal scrolling behavior if the command column cannot fit. Header sorting
will remain available for PID, USER, `%CPU`, `%MEM`, and COMMAND. The new
display-only fields will not add new sort controls or alter process actions.

## Testing

- Add parser coverage for a representative `ps` line containing all fields,
  including a command with spaces.
- Update process model tests to assert the new values are copied into the
  Slint row.
- Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test --locked`.

## Scope and compatibility

No dependency, persisted-config, or upstream-local contract changes are
needed. If a remote `ps` implementation cannot provide the requested fields,
the existing parser behavior of dropping malformed rows is preserved.
