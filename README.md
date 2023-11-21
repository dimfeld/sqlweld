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

# Installation

Installation currently just via `cargo install sqlweld`. More distribution coming soon.

# Example

This example shows a simple use of the tool, with two queries that share a permissions check partial.

## Input

### get_some_objects.sql.liquid

```liquid
SELECT * FROM some_objects
WHERE id=$[obj_id] AND team = $[team_id]
AND {% render 'perm_check', table: "'some_objects'" %}
```

### update_some_objects.sql.liquid

```liquid
UPDATE some_objects
SET value = 'a' 
WHERE id=$[obj_id] AND team = $[team_id]
AND {% render 'perm_check', action: "'write'", table: "'some_objects'" %}
```

### perm_check.partial.sql.liquid

```liquid
{%- unless user %}{% assign user = "$[user_id]" %}{% endunless -%}
{%- unless team  %}{% assign team = "$[team_id]" %}{% endunless -%}
{%- unless action %}{% assign action = "'read'" %}{% endunless -%}

EXISTS (
  SELECT 1
  FROM permissions
  WHERE user_id = {{ user }}
  AND team_id = {{ team }}
  AND action = {{ action }}
  AND object_type = {{table}}
)
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
