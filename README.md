# sqlweld

sqlweld is a CLI tool designed to help manage large libraries of SQL statements that need to reuse the same SQL clauses.

- Permissions checks often look very similar between queries, and updating these clauses is both tedious and a potential
    source of security bugs.
- Some queries need a number of slight variations, and while they can be formatted at runtime, this loses the benefits
    that come from your queries being statically defined, especially with tools that do compile-time checking like
    [sqlx](https://github.com/launchbadge/sqlx).

sqlweld is designed to help solve these problems. Query files are Liquid templates ending in the
`.sql.liquid` extension, and partials end with `.partial.sql.liquid`. The tool will render a `.sql` file for each
non-partial template it finds.

sqlweld is also a Rust library and can used from a `build.rs` file. By setting the `print_rerun_if_changed` option,
it will automatically print the appropriate statements to rerun if the queries change.
