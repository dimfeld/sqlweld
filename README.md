# sqlweld

sqlweld is a CLI tool designed to help manage large libraries of SQL statements that need to reuse the same SQL clauses.

- Permissions checks often look very similar between queries, and updating these clauses is both tedious and a potential
    source of security bugs.
- Some queries need a number of slight variations, and while they can be formatted at runtime, this loses the benefits
    that come from your queries being statically defined, especially with tools that do compile-time checking like
    [sqlx](https://github.com/launchbadge/sqlx).

sqlweld is designed to help solve these problems. Query files are [Tera](https://https://keats.github.io/tera/docs) templates ending in the
`.sql.tera` extension. The Tera syntax is similar, though not exactly the same as, Jinja.

Partials and macro files can end with `.macros.sql.tera` or `.partial.sql.tera`. The tool will render a `.sql` file for each
non-partial template it finds.

sqlweld is also a Rust library and can used from a `build.rs` file. By setting the `print_rerun_if_changed` option,
it will automatically print the appropriate statements to rerun if the queries change.

# Installation

Check the [releases page](https://github.com/dimfeld/sqlweld/releases) for Homebrew, npm, curl, and other options. Of course, `cargo install sqlweld` also works if you already have Rust installed.

# Watch Mode

Watch mode is not directly supported yet. Until it is, a tool such as [watchexec](https://watchexec.github.io/) can
accomplish the same functionality.

```shell
watchexec --exts tera -- sqlweld -v
```

# Example

This example shows a simple use of the tool, with two queries that share a permissions check partial.

## Input

### get_some_objects.sql.tera

```sql.jinja
{% import "perm_check.partial.sql.tera" as macros %}
SELECT * FROM some_objects
WHERE id=$[obj_id] AND team = $[team_id]
AND {{ macros::perm_check(table="'some_objects'") }}
```

### update_some_objects.sql.tera

```sql.jinja
{% import "perm_check.partial.sql.tera" as macros %}
UPDATE some_objects
SET value = 'a' 
WHERE id=$[obj_id] AND team = $[team_id]
AND {{ macros::perm_check(action="'write'", table="'some_objects'") }}
```

### perm_check.partial.sql.tera

```sql.jinja
{%- macro perm_check(user="$[user_id]", team="$[team_id]", action="'read'", table) -%}
EXISTS (
  SELECT 1
  FROM permissions
  WHERE user_id = {{ user }}
  AND team_id = {{ team }}
  AND action = {{ action }}
  AND object_type = {{table}}
)
{%- endmacro perm_check %}
```

## Output

### get_some_objects.sql

```sql
SELECT * FROM some_objects
WHERE id=$[obj_id] AND team = $[team_id]
AND EXISTS (
  SELECT 1
  FROM permissions
  WHERE user_id = $[user_id]
  AND team_id = $[team_id]
  AND action = 'read'
  AND object_type = 'some_objects'
)
```

### update_some_objects.sql

```sql
UPDATE some_objects
SET value = 'a' 
WHERE id=$[obj_id] AND team = $[team_id]
AND EXISTS (
  SELECT 1
  FROM permissions
  WHERE user_id = $[user_id]
  AND team_id = $[team_id]
  AND action = 'write'
  AND object_type = 'some_objects'
)
```
